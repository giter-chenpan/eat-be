use crate::jwtuser::Claims;
use rocket_okapi::{ openapi, JsonSchema };
use rocket::serde::{ json::{ Json, Value }, Deserialize, Serialize };
use reqwest;
use rocket::Config;
use scraper::{ ElementRef, Html, Selector };
use crate::common::data_structure::*;
use crate::common::enums::Code::*;

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Translation {
    destination: String,
    words: String,
}

#[derive(Deserialize, Serialize)]
struct Pronunciation {
    lang: String,
    source: String,
    pron: String,
}

#[derive(Deserialize, Serialize)]
struct Item {
    title: String,
    word_type_enum: Option<WordType>,
    pronunciation: Vec<Pronunciation>,
}

#[openapi(tag = "translation")]
#[post("/api/translation/words", data = "<data>", format = "json")]
pub async fn handle_translation(_claims: Claims, data: Json<Translation>) -> Value {
    let init_url = Config::figment().extract_inner::<String>("translation_url").unwrap();
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

    let document = Html::parse_document(&html);

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
            pronunciation: get_pronunciation(&i, &init_url),
        };
        res.push(item);
    }

    Rep::<Option<Vec<Item>>>::new(Success.self_code(), "成功", Some(Some(res)))
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
