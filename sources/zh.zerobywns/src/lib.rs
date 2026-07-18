#![no_std]
extern crate alloc;

use aidoku::{
	error::Result,
	helpers::{substring::Substring, uri::encode_uri},
	prelude::*,
	std::{json, net::Request, String, Vec},
	Chapter, Filter, FilterType, Manga, MangaContentRating, MangaPageResult, MangaStatus,
	MangaViewer, Page,
};
use alloc::string::ToString;

mod helper;

#[get_manga_list]
fn get_manga_list(filters: Vec<Filter>, page: i32) -> Result<MangaPageResult> {
	println!("[zh.zerobywns] get_manga_list page={}", page);
	let mut query = String::new();
	let mut odfie = String::from("addtime");
	let mut order = String::from("desc");

	for filter in filters {
		match filter.kind {
			FilterType::Title => {
				if let Ok(value) = filter.value.as_string() {
					query = value.read();
				}
			}
			FilterType::Sort => {
				if let Ok(value) = filter.value.as_object() {
					let index = value.get("index").as_int().unwrap_or(0);
					if index == 1 {
						odfie = String::from("edittime");
					}
					if value.get("ascending").as_bool().unwrap_or(false) {
						order = String::from("asc");
					}
				}
			}
			_ => continue,
		}
	}

	let mut mangas: Vec<Manga> = Vec::new();

	if query.is_empty() {
		let mut url = format!(
			"{}/plugin.php?id=jameson_manhua&c=index&a=ku",
			helper::get_url()
		);

		url.push_str(&format!("&odfie={}&order={}&page={}", odfie, order, page));

		let html = helper::html(url)?;

		for item in html.select(".uk-card").array() {
			let item = match item.as_node() {
				Ok(node) => node,
				Err(_) => continue,
			};
			let id = item
				.select("div:nth-child(1)>a")
				.attr("href")
				.read()
				.split("=")
				.map(|a| a.to_string())
				.collect::<Vec<String>>()
				.pop()
				.unwrap_or_default();
			let cover =
				helper::absolute_url(&item.select("div:nth-child(1)>a>img").attr("src").read());
			let title = item
				.select("div:nth-child(2)>p>a")
				.text()
				.read()
				.trim()
				.to_string();
			if id.is_empty() || title.is_empty() {
				continue;
			}
			mangas.push(Manga {
				id,
				cover,
				title,
				nsfw: MangaContentRating::Suggestive,
				..Default::default()
			});
		}
	} else {
		let url = format!(
			"{}/plugin.php?id=jameson_manhua&c=index&a=search&keyword={}&page={}",
			helper::get_url(),
			encode_uri(query),
			page
		);
		let html = helper::html(url)?;

		for item in html.select(".uk-card").array() {
			let item = match item.as_node() {
				Ok(node) => node,
				Err(_) => continue,
			};
			let id = item
				.attr("href")
				.read()
				.split("=")
				.map(|a| a.to_string())
				.collect::<Vec<String>>()
				.pop()
				.unwrap_or_default();
			let cover =
				helper::absolute_url(&item.select("div:nth-child(1)>img").attr("src").read());
			let title = item
				.select("div:nth-child(2)>p")
				.text()
				.read()
				.trim()
				.to_string();
			if id.is_empty() || title.is_empty() {
				continue;
			}
			mangas.push(Manga {
				id,
				cover,
				title,
				nsfw: MangaContentRating::Suggestive,
				..Default::default()
			});
		}
	}

	println!("[zh.zerobywns] manga count={}", mangas.len());
	Ok(MangaPageResult {
		has_more: !mangas.is_empty(),
		manga: mangas,
	})
}

#[get_manga_details]
fn get_manga_details(id: String) -> Result<Manga> {
	println!("[zh.zerobywns] get_manga_details id={}", id);
	let url = format!(
		"{}/plugin.php?id=jameson_manhua&c=index&a=bofang&kuid={}",
		helper::get_url(),
		id.clone()
	);
	let html = helper::html(url.clone())?;
	let cover = helper::absolute_url(&html.select(".uk-width-medium>img").attr("src").read());
	let title = html.select(".uk-margin-left>ul>li>h3").text().read();
	let author = html
		.select(".uk-margin-left>ul>li>.cl>a[href*='zuozhe']")
		.text()
		.read()
		.replace("作者:", "")
		.split("×")
		.map(|a| a.to_string())
		.collect::<Vec<String>>()
		.join(", ");
	let description = html
		.select(".uk-margin-left>ul>li>.uk-alert")
		.text()
		.read()
		.trim()
		.to_string();
	let categories = html
		.select(".uk-margin-left>ul>li>.cl>a[href*='category']")
		.array()
		.filter_map(|a| a.as_node().ok().map(|node| node.text().read()))
		.collect::<Vec<String>>();
	let status = match html
		.select(".uk-margin-left>ul>li>.cl>span:nth-child(6)")
		.text()
		.read()
		.as_str()
	{
		"连载中" => MangaStatus::Ongoing,
		"已完结" => MangaStatus::Completed,
		_ => MangaStatus::Unknown,
	};

	Ok(Manga {
		id,
		cover,
		title,
		author,
		artist: String::new(),
		description,
		url,
		categories,
		status,
		nsfw: MangaContentRating::Suggestive,
		viewer: MangaViewer::Rtl,
	})
}

#[get_chapter_list]
fn get_chapter_list(id: String) -> Result<Vec<Chapter>> {
	println!("[zh.zerobywns] get_chapter_list id={}", id);
	let url = format!(
		"{}/plugin.php?id=jameson_manhua&c=index&a=bofang&kuid={}",
		helper::get_url(),
		id
	);
	let html = helper::html(url)?;
	let list = html.select(".muludiv>a").array();
	let mut chapters: Vec<Chapter> = Vec::new();

	for (index, item) in list.enumerate() {
		let item = match item.as_node() {
			Ok(item) => item,
			Err(_) => continue,
		};
		let id = item
			.attr("href")
			.read()
			.split("=")
			.map(|a| a.to_string())
			.collect::<Vec<String>>()
			.pop()
			.unwrap_or_default();
		if id.is_empty() {
			continue;
		}
		let title = item.text().read();
		let url = format!(
			"{}/plugin.php?id=jameson_manhua&a=read&zjid={}",
			helper::get_url(),
			id.clone()
		);
		chapters.push(Chapter {
			id,
			title,
			chapter: (index + 1) as f32,
			url,
			..Default::default()
		});
	}
	chapters.reverse();
	println!("[zh.zerobywns] chapter count={}", chapters.len());

	Ok(chapters)
}

#[get_page_list]
fn get_page_list(_: String, chapter_id: String) -> Result<Vec<Page>> {
	println!("[zh.zerobywns] get_page_list chapter_id={}", chapter_id);
	let url = format!(
		"{}/plugin.php?id=jameson_manhua&a=read&zjid={}",
		helper::get_url(),
		chapter_id
	);
	let html = helper::html(url)?;
	let text = html.html().read();
	let mut pages: Vec<Page> = Vec::new();
	let list = text
		.substring_after("let listimg=")
		.unwrap_or("[]")
		.substring_before(";")
		.unwrap_or("[]");
	let list = match json::parse(list).and_then(|value| value.as_array()) {
		Ok(list) => list,
		Err(_) => return Ok(pages),
	};

	for (index, item) in list.enumerate() {
		let item = match item.as_object() {
			Ok(node) => node,
			Err(_) => continue,
		};
		let url = helper::absolute_url(&item.get("file").as_string()?.read());
		pages.push(Page {
			index: index as i32,
			url,
			..Default::default()
		})
	}

	println!("[zh.zerobywns] page count={}", pages.len());
	Ok(pages)
}

#[modify_image_request]
fn modify_image_request(request: Request) {
	let referer = helper::get_url();
	let cookie = helper::get_cookie();
	let mut request = request
		.header("User-Agent", helper::USER_AGENT)
		.header("Referer", &referer);
	if !cookie.is_empty() {
		request = request.header("Cookie", &cookie);
	}
	let _ = request;
}
