#![no_std]

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, HashMap, Home, HomeComponent,
	HomeComponentValue, HomeLayout, ImageRequestProvider, Listing, ListingKind, ListingProvider,
	Manga, MangaPageResult, MangaStatus, Page, PageContent, PageContext, Result, Source,
	UpdateStrategy, Viewer,
	alloc::{String, Vec, format, string::ToString, vec},
	helpers::{
		string::StripPrefixOrSelf,
		uri::{QueryParameters, encode_uri_component},
	},
	imports::{
		html::{Document, Element},
		net::Request,
		std::parse_date,
	},
	prelude::*,
};

const BASE_URL: &str = "https://www.xiurenai.com";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";

struct XiurenAi;

impl Source for XiurenAi {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut category_path = String::new();
		let mut text_query = query.unwrap_or_default();
		let mut tag_filter = String::new();

		for filter in filters {
			match filter {
				FilterValue::Select { id, value } if id == "category" && !value.is_empty() => {
					category_path = value;
				}
				FilterValue::Text { id, value } if id == "tag" && !value.trim().is_empty() => {
					tag_filter = value.trim().to_string();
				}
				FilterValue::Text { id, value } if id == "author" && !value.trim().is_empty() => {
					if !text_query.trim().is_empty() {
						text_query.push(' ');
					}
					text_query.push_str(value.trim());
				}
				_ => {}
			}
		}

		let url = if !text_query.trim().is_empty() {
			search_url(&text_query, page)
		} else if !tag_filter.is_empty() {
			tag_url(&tag_filter, page)
		} else if !category_path.is_empty() {
			paged_path_url(&category_path, page)
		} else {
			latest_url(page)
		};

		let html = get_html(&url)?;
		Ok(parse_manga_list(&html))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = manga_url(&manga.key);
		let html = get_html(&url)?;

		let title = html
			.select_first("#single-content .article-title")
			.and_then(|el| el.text())
			.map(|text| text.trim().to_string())
			.filter(|text| !text.is_empty())
			.unwrap_or_else(|| manga.title.clone());
		let date_uploaded = parse_detail_date(&html);

		if needs_details {
			let category = html
				.select_first("#single-content .article-meta .item-cats a")
				.and_then(|el| el.text())
				.map(|text| text.trim().to_string())
				.filter(|text| !text.is_empty());
			let tags = parse_detail_tags(&html);
			let authors = authors_from_title_and_tags(&title, category.as_deref(), &tags);
			let cover = first_article_image(&html).or(manga.cover);
			let mut description_parts = Vec::new();

			if let Some(category) = &category {
				description_parts.push(format!("机构：{category}"));
			}
			if let Some(date) = detail_date_text(&html) {
				description_parts.push(format!("发布日期：{date}"));
			}
			if let Some(intro) = html
				.select_first("#single-content .article-content")
				.and_then(|el| el.own_text())
				.map(|text| text.trim().to_string())
				.filter(|text| !text.is_empty())
			{
				description_parts.push(intro);
			}
			description_parts.push(format!("原网站链接：{url}"));

			manga.title = title.clone();
			manga.cover = cover;
			manga.authors = if authors.is_empty() {
				None
			} else {
				Some(authors)
			};
			manga.tags = if tags.is_empty() {
				category.map(|category| vec![category])
			} else {
				Some(tags)
			};
			manga.description = Some(description_parts.join("\n"));
			manga.status = MangaStatus::Completed;
			manga.url = Some(url.clone());
			manga.viewer = Viewer::Webtoon;
			manga.update_strategy = UpdateStrategy::Never;
		}

		if needs_chapters {
			manga.chapters = Some(vec![Chapter {
				key: manga.key.clone(),
				title: Some("完整图集".into()),
				chapter_number: Some(1.0),
				date_uploaded,
				url: Some(url),
				..Default::default()
			}]);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = chapter.url.unwrap_or_else(|| {
			if chapter.key.is_empty() {
				manga_url(&manga.key)
			} else {
				manga_url(&chapter.key)
			}
		});
		let html = get_html(&url)?;
		let mut pages = Vec::new();
		let mut seen = Vec::<String>::new();

		if let Some(images) = html.select("#single-content .article-content img") {
			for image in images {
				let Some(image_url) = image_url(&image) else {
					continue;
				};
				if !is_valid_image_url(&image_url) || seen.contains(&image_url) {
					continue;
				}
				seen.push(image_url.clone());

				let mut context: PageContext = HashMap::new();
				context.insert("referer".into(), url.clone());
				pages.push(Page {
					content: PageContent::url_context(image_url, context),
					..Default::default()
				});
			}
		}

		Ok(pages)
	}
}

impl ListingProvider for XiurenAi {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			"latest" => {
				let html = get_html(&latest_url(page))?;
				Ok(parse_manga_list(&html))
			}
			_ => bail!("Invalid listing"),
		}
	}
}

impl Home for XiurenAi {
	fn get_home(&self) -> Result<HomeLayout> {
		let html = get_html(&latest_url(1))?;
		let entries = parse_manga_list(&html).entries;

		if entries.is_empty() {
			return Ok(HomeLayout::default());
		}

		Ok(HomeLayout {
			components: vec![HomeComponent {
				title: Some("最新发布".into()),
				subtitle: None,
				value: HomeComponentValue::Scroller {
					entries: entries.into_iter().map(Into::into).collect(),
					listing: Some(Listing {
						id: "latest".into(),
						name: "最新发布".into(),
						kind: ListingKind::Default,
					}),
				},
			}],
		})
	}
}

impl ImageRequestProvider for XiurenAi {
	fn get_image_request(&self, url: String, context: Option<PageContext>) -> Result<Request> {
		let referer = context
			.as_ref()
			.and_then(|ctx| ctx.get("referer").map(|value| value.as_str()))
			.unwrap_or(BASE_URL);
		Ok(Request::get(url)?
			.header("User-Agent", USER_AGENT)
			.header("Referer", referer))
	}
}

impl DeepLinkHandler for XiurenAi {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if !url.starts_with(BASE_URL) || !url.ends_with(".html") {
			return Ok(None);
		}
		Ok(Some(DeepLinkResult::Manga {
			key: key_from_url(&url),
		}))
	}
}

fn get_html(url: &str) -> Result<Document> {
	Ok(Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Referer", BASE_URL)
		.html()?)
}

fn latest_url(page: i32) -> String {
	if page <= 1 {
		format!("{BASE_URL}/all")
	} else {
		format!("{BASE_URL}/all/page/{page}")
	}
}

fn paged_path_url(path: &str, page: i32) -> String {
	if page <= 1 {
		format!("{BASE_URL}{path}")
	} else {
		format!("{BASE_URL}{path}/page/{page}")
	}
}

fn search_url(query: &str, page: i32) -> String {
	let mut qs = QueryParameters::new();
	qs.push("s", Some(query));
	if page <= 1 {
		format!("{BASE_URL}/?{qs}")
	} else {
		format!("{BASE_URL}/page/{page}?{qs}")
	}
}

fn tag_url(tag: &str, page: i32) -> String {
	let encoded = encode_uri_component(tag.trim());
	if page <= 1 {
		format!("{BASE_URL}/tag/{encoded}")
	} else {
		format!("{BASE_URL}/tag/{encoded}/page/{page}")
	}
}

fn parse_manga_list(html: &Document) -> MangaPageResult {
	let entries = html
		.select(".content .post.grid")
		.map(|items| {
			let mut entries = Vec::new();
			let mut seen_keys = Vec::<String>::new();

			for item in items {
				let Some(link) = item.select_first("h3 a, .img > a") else {
					continue;
				};
				let Some(url) = link.attr("abs:href") else {
					continue;
				};
				if !url.ends_with(".html") {
					continue;
				}

				let key = key_from_url(&url);
				if key.is_empty() || seen_keys.contains(&key) {
					continue;
				}
				seen_keys.push(key.clone());

				let title = link
					.attr("title")
					.or_else(|| link.text())
					.unwrap_or_default()
					.trim()
					.to_string();
				if title.is_empty() {
					continue;
				}

				let cover = item
					.select_first(".img img")
					.and_then(|img| image_url(&img));
				let tags = collect_link_texts(&item, ".tag a");
				let category = item
					.select_first(".img-cat a")
					.and_then(|el| el.text())
					.map(|text| text.trim().to_string())
					.filter(|text| !text.is_empty());
				let authors = authors_from_title_and_tags(&title, category.as_deref(), &tags);

				entries.push(Manga {
					key,
					title,
					cover,
					authors: if authors.is_empty() {
						None
					} else {
						Some(authors)
					},
					tags: if tags.is_empty() { None } else { Some(tags) },
					url: Some(url),
					..Default::default()
				});
			}

			entries
		})
		.unwrap_or_default();

	let has_next_page = html.select_first(".pagination .next-page a").is_some();

	MangaPageResult {
		entries,
		has_next_page,
	}
}

fn parse_detail_tags(html: &Document) -> Vec<String> {
	collect_link_texts(html, "#single-content .article-tags a")
}

fn collect_link_texts<T: SelectExt>(root: &T, selector: &str) -> Vec<String> {
	let mut values = Vec::new();
	if let Some(elements) = root.select(selector) {
		for element in elements {
			let value = element.text().unwrap_or_default().trim().to_string();
			if !value.is_empty() && !values.contains(&value) {
				values.push(value);
			}
		}
	}
	values
}

trait SelectExt {
	fn select(&self, selector: &str) -> Option<aidoku::imports::html::ElementList>;
}

impl SelectExt for Document {
	fn select(&self, selector: &str) -> Option<aidoku::imports::html::ElementList> {
		self.select(selector)
	}
}

impl SelectExt for Element {
	fn select(&self, selector: &str) -> Option<aidoku::imports::html::ElementList> {
		self.select(selector)
	}
}

fn first_article_image(html: &Document) -> Option<String> {
	html.select_first("#single-content .article-content img")
		.and_then(|img| image_url(&img))
}

fn image_url(image: &Element) -> Option<String> {
	for attr in [
		"abs:data-original",
		"abs:data-src",
		"abs:data-lazy-src",
		"abs:src",
	] {
		if let Some(url) = image.attr(attr).filter(|url| !url.trim().is_empty()) {
			return Some(url);
		}
	}

	image
		.attr("srcset")
		.and_then(|srcset| best_srcset_url(&srcset))
}

fn best_srcset_url(srcset: &str) -> Option<String> {
	let mut best_url = None;
	let mut best_width = 0;
	for part in srcset.split(',') {
		let mut pieces = part.split_whitespace();
		let Some(url) = pieces.next() else {
			continue;
		};
		let width = pieces
			.next()
			.and_then(|value| value.strip_suffix('w'))
			.and_then(|value| value.parse::<i32>().ok())
			.unwrap_or(0);
		if best_url.is_none() || width > best_width {
			best_width = width;
			best_url = Some(to_absolute_url(url));
		}
	}
	best_url
}

fn is_valid_image_url(url: &str) -> bool {
	let lower = url.to_lowercase();
	!url.trim().is_empty()
		&& !lower.starts_with("data:")
		&& !lower.contains("logo")
		&& !lower.contains("avatar")
		&& !lower.contains("placeholder")
		&& !lower.contains("loading")
		&& !lower.contains("lazy.gif")
}

fn to_absolute_url(url: &str) -> String {
	if url.starts_with("//") {
		format!("https:{url}")
	} else if url.starts_with('/') {
		format!("{BASE_URL}{url}")
	} else {
		url.to_string()
	}
}

fn key_from_url(url: &str) -> String {
	url.strip_prefix_or_self(BASE_URL)
		.trim_start_matches('/')
		.to_string()
}

fn manga_url(key: &str) -> String {
	if key.starts_with("http://") || key.starts_with("https://") {
		key.to_string()
	} else if key.starts_with('/') {
		format!("{BASE_URL}{key}")
	} else {
		format!("{BASE_URL}/{key}")
	}
}

fn detail_date_text(html: &Document) -> Option<String> {
	html.select_first("#single-content .article-meta .item:has(.icon-time)")
		.and_then(|el| el.text())
		.map(|text| text.trim().to_string())
		.filter(|text| !text.is_empty())
}

fn parse_detail_date(html: &Document) -> Option<i64> {
	detail_date_text(html).and_then(|text| parse_date(text, "yyyy-MM-dd"))
}

fn authors_from_title_and_tags(
	title: &str,
	category: Option<&str>,
	tags: &[String],
) -> Vec<String> {
	let mut authors = Vec::new();
	if let Some(model) = model_from_title(title) {
		push_unique(&mut authors, &model);
	}
	for tag in tags {
		if Some(tag.as_str()) != category && looks_like_model_tag(tag) {
			push_unique(&mut authors, tag);
		}
	}
	authors
}

fn model_from_title(title: &str) -> Option<String> {
	let mut tail = title.trim();

	if let Some(index) = tail.rfind("No.") {
		tail = &tail[index + 3..];
	} else if let Some(index) = tail.rfind("NO.") {
		tail = &tail[index + 3..];
	} else if let Some(index) = tail.rfind("Vol.") {
		tail = &tail[index + 4..];
	} else if let Some(index) = tail.rfind(' ') {
		tail = &tail[index + 1..];
	}

	let tail = tail.trim();
	let start = tail
		.char_indices()
		.find(|(_, ch)| !ch.is_ascii_digit() && *ch != '.' && *ch != ' ')
		.map(|(index, _)| index)
		.unwrap_or(0);
	let model = tail[start..].trim();
	if model.is_empty() || model.contains("合集") {
		None
	} else {
		Some(model.to_string())
	}
}

fn looks_like_model_tag(tag: &str) -> bool {
	tag.chars().any(|ch| ch.is_ascii_alphabetic()) || tag.chars().count() <= 6
}

fn push_unique(values: &mut Vec<String>, value: &str) {
	let value = value.trim();
	if !value.is_empty() && !values.iter().any(|existing| existing == value) {
		values.push(value.to_string());
	}
}

register_source!(
	XiurenAi,
	Home,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler
);
