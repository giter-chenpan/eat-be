use crate::jwtuser::Claims;
use rocket_okapi::{ openapi, JsonSchema };
use rocket::serde::{ json::{ serde_json, Json }, Deserialize, Serialize };
use reqwest;
use crate::pool::Db;
use uuid::{ ContextV7, Timestamp, Uuid };
use  crate::config::get_config;
use scraper::{ ElementRef, Html, Selector };
use crate::common::{ data_structure::*, enums::Code::* };
use sea_orm_rocket::Connection;
use sea_orm::{ ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set };
use ::entity::words::{ self, Entity as Words };

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Translation {
    destination: String,
    words: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
struct Pronunciation {
    lang: String,
    source: String,
    pron: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
struct ItemExample {
    label: String,
    value: Option<String>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
struct WordTranslation {
    word: Option<String>,
    examples: Vec<ItemExample>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Item {
    title: String,
    translation: Vec<WordTranslation>,
    word_type_enum: Option<WordType>,
    pronunciation: Vec<Pronunciation>,
}
#[openapi(tag = "translation", ignore = "db")]
#[post("/api/translation/words", data = "<data>", format = "json")]
pub async fn handle_translation(
    _claims: Claims,
    db: Connection<'_, Db>,
    data: Json<Translation>
) -> Json<Rep<Option<Vec<Item>>>> {
    let db = db.into_inner();
    let result = Words::find().filter(words::Column::Word.eq(&data.words)).one(db).await;
    let trans = match result {
        Ok(Some(trans)) => trans.translation,
        Ok(None) => {
            println!("result none");
            "".to_string()
        }
        Err(error) => {
            println!("translation error: {}", error);
            "".to_string()
        }
    };

    if !trans.is_empty() {
        let translation = serde_json::from_str(trans.as_str());

        match translation {
            Ok(translation) => {
                return Rep::<Option<Vec<Item>>>::new(
                    Success.self_code(),
                    "成功",
                    Some(translation)
                );
            }
            Err(error) => {
                println!("translation error: {}", error);
                return Rep::<Option<Vec<Item>>>::new(BusinessError.self_code(), "解析错误", None);
            }
        }
    }

    let init_url = get_config().translation_url.clone();
    let mut url = init_url.clone();
    match data.destination.as_str() {
        "en" => {
            url = url + "/dictionary/chinese-simplified-english/" + &data.words;
           
        }
        "zh" => {
            url = url + "/dictionary/english-chinese-simplified/" + &data.words;
        }
        _ => {
            return Rep::<Option<Vec<Item>>>::new(BadRequest.self_code(), "参数错误", None);
        }
    }
    let html = reqwest::get(url).await.unwrap().text().await.unwrap();

    // 在同步上下文中处理 HTML 解析
    let res = parse_html(&html, &init_url);

    if res.is_empty() {
        return Rep::<Option<Vec<Item>>>::new(Success.self_code(), "成功", Some(Some(vec![])));
    }
    let ts = Timestamp::now(ContextV7::new());
    let save_rep = (words::ActiveModel {
        id: Set(Uuid::new_v7(ts).to_string()),
        word: Set(data.words.to_string()),
        translation: Set(serde_json::to_string(&res).unwrap()),
        create_user: Set(_claims.sub.to_owned()),
        r#type: Set(data.destination.to_string()),
    }).insert(db).await;

    match save_rep {
        Ok(_) => Rep::<Option<Vec<Item>>>::new(Success.self_code(), "成功", Some(Some(res))),
        Err(error) => {
            println!("save error: {}", error);
            Rep::<Option<Vec<Item>>>::new(BusinessError.self_code(), &format!("数据库错误"), None)
        }
    }
}

// 同步函数处理 HTML 解析
fn parse_html(html: &str, init_url: &str) -> Vec<Item> {
    let document = Html::parse_document(html);
    let origin_selector = Selector::parse(".pr.entry-body__el").unwrap();
    let title_selector = Selector::parse(".hw.dhw").unwrap();
    let type_selector = Selector::parse(".pos.dpos").unwrap();

    let mut res: Vec<Item> = vec![];

    for i in document.select(&origin_selector) {
        let title = i.select(&title_selector).next().unwrap();
        let word_type = i.select(&type_selector).next().unwrap();

        let item = Item {
            title: title.inner_html(),
            word_type_enum: mapping_word_type(&word_type.inner_html()),
            pronunciation: get_pronunciation(&i, init_url),
            translation: get_translation(&i),
        };
        res.push(item);
    }
    res
}

fn get_translation(i: &ElementRef<'_>) -> Vec<WordTranslation> {
    let trans_selector = Selector::parse(".def-body.ddef_b").unwrap();
    let word_selector = Selector::parse(".trans.dtrans.dtrans-se").unwrap();
    let mut res: Vec<WordTranslation> = vec![];

    fn get_examples(i: &ElementRef<'_>) -> Vec<ItemExample> {
        let mut inner_res: Vec<ItemExample> = vec![];
        let example_selector = Selector::parse(".examp.dexamp").unwrap();
        for example in i.select(&example_selector) {
            let label_selector = Selector::parse(".eg.deg").unwrap();
            let value_selector = Selector::parse(".trans.dtrans.dtrans-se.hdb.break-cj").unwrap();
            inner_res.push(ItemExample {
                label: example
                    .select(&label_selector)
                    .next()
                    .unwrap()
                    .text()
                    .collect::<Vec<_>>()
                    .join(""),
                value: example
                    .select(&value_selector)
                    .next()
                    .map_or(None, |s| Some(s.inner_html())),
            });
        }
        inner_res
    }
    for trans in i.select(&trans_selector) {
        res.push(WordTranslation {
            word: trans
                .select(&word_selector)
                .next()
                .map_or(None, |s| Some(s.text().collect::<Vec<_>>().join(""))),
            examples: get_examples(&trans),
        });
    }
    res
}

fn get_pronunciation(i: &ElementRef<'_>, url: &str) -> Vec<Pronunciation> {
    let pron_selector = Selector::parse(".dpron-i").unwrap();

    let mut res: Vec<Pronunciation> = vec![];
    for pron in i.select(&pron_selector) {
        let lang_selector = Selector::parse(".region.dreg").unwrap();
        let source_selector = Selector::parse(".daud source").unwrap();
        let pron_selector = Selector::parse(".pron.dpron").unwrap();

        let lang = pron.select(&lang_selector).next().unwrap();
        let source = pron.select(&source_selector).next().unwrap();
        let pron = pron.select(&pron_selector).next().unwrap();

        res.push(Pronunciation {
            lang: lang.inner_html(),
            pron: pron.text().collect::<Vec<_>>().join(""),
            source: url.to_string() + source.value().attr("src").unwrap(),
        });
    }
    res
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct WordsRepItem {
    beta: bool,
    url: String,
    word: String,
}

#[openapi(tag = "translation")]
#[post("/api/translation/getWords", data = "<data>", format = "json")]
pub async fn get_words(
    _claims: Claims,
    data: Json<Translation>
) -> Json<Rep<Option<Vec<WordsRepItem>>>> {
    let init_url = get_config().translation_url.clone();
    let mut url = init_url.clone();

    match data.destination.as_str() {
        "zh" => {
            url =
                url +
                "/autocomplete/amp?dataset=english-chinese-simplified&__amp_source_origin=" +
                &init_url +
                "&q=" +
                &data.words;
        }
        "en" => {
            url =
                url +
                "/autocomplete/amp?dataset=chinese-simplified-english&__amp_source_origin=" +
                &init_url +
                "&q=" +
                &data.words;
        }
        _ => {
            return Rep::<Option<Vec<WordsRepItem>>>::new(BadRequest.self_code(), "参数错误", None);
        }
    }

    let words_rep = reqwest::get(url).await.unwrap().json::<Vec<WordsRepItem>>().await;

    match words_rep {
        Ok(words_rep) => {
            Rep::<Option<Vec<WordsRepItem>>>::new(
                Success.self_code(),
                "成功",
                Some(Some(words_rep))
            )
        }
        Err(error) => {
            println!("{}", error);
            Rep::<Option<Vec<WordsRepItem>>>::new(BusinessError.self_code(), "查询错误", None)
        }
    }
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct FindPageRepItem {
    id: String,
    word: String,
    translations: Vec<Item>,
    create_user: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct FindPageRep {
    page: u64,
    total: u64,
    list: Vec<FindPageRepItem>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FindPageParams {
    pub page: u64,
    pub page_size: u64,
    pub translation_type: String,
}


#[openapi(tag = "translation", ignore = "db")]
#[post("/api/translation/findPage", data = "<data>", format = "json")]
pub async fn find_page(
    _claims: Claims,
    db: Connection<'_, Db>,
    data: Json<FindPageParams>
) -> Json<Rep<FindPageRep>> {
    let db = db.into_inner();
    let  result = Words::find().filter(words::Column::Type.eq(&data.translation_type)).paginate(db, data.page_size);
    let current_page = data.page - 1;

    let result_list = result.fetch_page(current_page).await.unwrap().into_iter().map(|item| FindPageRepItem {
        id: item.id,
        word: item.word,
        translations: serde_json::from_str(&item.translation).unwrap(),
        create_user: item.create_user,
    }).collect::<Vec<FindPageRepItem>>();

    let rep = FindPageRep {
        page: data.page,
        total: match result.num_items().await {
            Ok(total) => total,
            Err(error) => {
                println!("{}", error);
                return Rep::<FindPageRep>::new(BusinessError.self_code(), "查询错误", None);
            }
        },
        list: result_list,
    };

    Rep::<FindPageRep>::new(Success.self_code(), "成功", Some(rep))
}
