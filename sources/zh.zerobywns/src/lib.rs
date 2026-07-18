#![no_std]
extern crate alloc;

use aidoku::{
	error::Result,
	helpers::uri::encode_uri,
	prelude::*,
	std::{net::Request, String, Vec},
	Chapter, Filter, FilterType, Manga, MangaContentRating, MangaPageResult, MangaStatus,
	MangaViewer, Page,
};
use alloc::string::ToString;

mod helper;

fn query_value(value: String, key: &str) -> Option<String> {
	value
		.split('?')
		.nth(1)
		.unwrap_or(&value)
		.split('&')
		.find_map(|part| {
			let mut parts = part.split('=');
			if parts.next() == Some(key) {
				parts.next().map(|value| value.to_string())
			} else {
				None
			}
		})
		.filter(|value| !value.is_empty())
}

fn first_text(node: &aidoku::std::html::Node, selector: &str) -> String {
	node.select(selector).text().read().trim().to_string()
}

fn first_attr(node: &aidoku::std::html::Node, selector: &str, attr: &str) -> String {
	node.select(selector).attr(attr).read()
}

#[get_manga_list]
fn get_manga_list(filters: Vec<Filter>, page: i32) -> Result<MangaPageResult> {
	let page = page.max(1);
	let mut query = String::new();
	for filter in filters {
		if filter.kind == FilterType::Title {
			if let Ok(value) = filter.value.as_string() {
				query = value.read();
			}
		}
	}

	let url = if query.trim().is_empty() {
		format!(
			"{}/pc/pc/?order=addtime&dir=desc&page={}",
			helper::get_url(),
			page
		)
	} else {
		format!(
			"{}/pc/pc/?keyword={}&order=addtime&dir=desc&page={}",
			helper::get_url(),
			encode_uri(query),
			page
		)
	};
	let html = helper::html(url)?;
	let mut manga: Vec<Manga> = Vec::new();

	for item in html.select("a.group.block[href*='kuid=']").array() {
		let item = match item.as_node() {
			Ok(item) => item,
			Err(_) => continue,
		};
		let href = item.attr("href").read();
		let id = match query_value(href, "kuid") {
			Some(id) => id,
			None => continue,
		};
		let title = first_text(&item, "h3.manga-card-title");
		if title.is_empty() {
			continue;
		}
		let cover = helper::absolute_url(&first_attr(&item, "img", "src"));
		manga.push(Manga {
			id,
			cover,
			title,
			nsfw: MangaContentRating::Suggestive,
			..Default::default()
		});
	}

	Ok(MangaPageResult {
		has_more: !manga.is_empty(),
		manga,
	})
}

#[get_manga_details]
fn get_manga_details(id: String) -> Result<Manga> {
	let url = format!("{}/pc/details/?kuid={}", helper::get_url(), id.clone());
	let html = helper::html(url.clone())?;
	let mut author = String::new();
	let mut categories = Vec::new();
	let mut status = MangaStatus::Unknown;

	for span in html.select("span.px-3.py-1.bg-gray-100").array() {
		let span = match span.as_node() {
			Ok(span) => span,
			Err(_) => continue,
		};
		let text = span.text().read().trim().to_string();
		if text.is_empty() {
			continue;
		}
		if text.starts_with("作者:") {
			author = text.replace("作者:", "").trim().to_string();
			continue;
		}
		if text == "连载中" {
			status = MangaStatus::Ongoing;
			continue;
		}
		if text == "已完结" {
			status = MangaStatus::Completed;
			continue;
		}
		if text.starts_with("收藏:") || text.starts_with("人气:") {
			continue;
		}
		categories.push(text);
	}

	Ok(Manga {
		id,
		cover: helper::absolute_url(&first_attr(&html, "img[src*='tupa.zerobyw33.com']", "src")),
		title: first_text(&html, "h1.text-2xl.font-medium"),
		author,
		artist: String::new(),
		description: first_text(&html, "p[x-ref='summaryText']"),
		url,
		categories,
		status,
		nsfw: MangaContentRating::Suggestive,
		viewer: MangaViewer::Rtl,
	})
}

#[get_chapter_list]
fn get_chapter_list(id: String) -> Result<Vec<Chapter>> {
	let url = format!("{}/pc/details/?kuid={}", helper::get_url(), id);
	let html = helper::html(url)?;
	let mut chapters: Vec<Chapter> = Vec::new();
	let mut last_zjid = 0;

	for item in html.select(".grid > *").array() {
		let item = match item.as_node() {
			Ok(item) => item,
			Err(_) => continue,
		};
		let title = item.text().read().trim().to_string();
		let href = item.attr("href").read();
		let chapter_id = if href.starts_with("/pc/view/index.php?zjid=") {
			match query_value(href, "zjid") {
				Some(id) => {
					if let Ok(num) = id.parse::<i64>() {
						last_zjid = num;
					}
					id
				}
				None => continue,
			}
		} else {
			last_zjid += 1;
			last_zjid.to_string()
		};
		let chapter = (chapters.len() + 1) as f32;
		chapters.push(Chapter {
			id: chapter_id.clone(),
			title,
			chapter,
			url: format!(
				"{}/pc/view/index.php?zjid={}",
				helper::get_url(),
				chapter_id
			),
			..Default::default()
		});
	}

	chapters.reverse();
	Ok(chapters)
}

#[get_page_list]
fn get_page_list(_: String, chapter_id: String) -> Result<Vec<Page>> {
	let url = format!(
		"{}/pc/view/index.php?zjid={}",
		helper::get_url(),
		chapter_id
	);
	let html = helper::html(url)?;
	let mut pages: Vec<Page> = Vec::new();

	for img in html.select("img.manga-image").array() {
		let img = match img.as_node() {
			Ok(img) => img,
			Err(_) => continue,
		};
		let url = helper::absolute_url(&img.attr("src").read());
		if url.is_empty() {
			continue;
		}
		pages.push(Page {
			index: pages.len() as i32,
			url,
			..Default::default()
		});
	}

	Ok(pages)
}

#[modify_image_request]
fn modify_image_request(request: Request) {
	let referer = format!("{}/", helper::get_url());
	let _ = request
		.header("User-Agent", helper::USER_AGENT)
		.header("Referer", &referer);
}
