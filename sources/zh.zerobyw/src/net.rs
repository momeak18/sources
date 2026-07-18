use aidoku::{
	FilterValue, Result,
	alloc::{String, format, string::ToString},
	error,
	helpers::uri::{QueryParameters, encode_uri_component},
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
	imports::net::{HttpMethod, Request},
};
use alloc::vec::Vec;

use crate::net::Url::SearchOrFilter;
use core::fmt::{Display, Formatter, Result as FmtResult};
use strum::{Display, EnumIs};

const API_URL: &str = "https://www.zerobyw33.com";
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
    AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0.1 Safari/605.1.15";

#[derive(Display, EnumIs)]
pub enum Url<'a> {
	#[strum(to_string = "/pc/pc/?{0}")]
	SearchOrFilter(SearchOrFilterQuery),
	#[strum(to_string = "/pc/details/?kuid={key}")]
	Manga { key: &'a str },
	#[strum(to_string = "/pc/view/index.php?zjid={key}")]
	Chapter { key: &'a str },
	#[strum(to_string = "/member.php?mod=logging&action=login")]
	Login,
	#[strum(to_string = "/pc/pc/")]
	Home,
	#[strum(to_string = "{key}")]
	Logout { key: &'a str },
}

impl Url<'_> {
	pub fn to_string(&self) -> Result<String> {
		let base_url = API_URL;
		Ok(format!("{base_url}{self}"))
	}

	pub fn request(&self, method: HttpMethod) -> Result<Request> {
		let url = self.to_string()?;
		let mut request = Request::new(url, method)?
			.header("User-Agent", USER_AGENT)
			.header("Referer", API_URL);
		let cookie = get_cookie();
		if !cookie.is_empty() {
			request.set_header("Cookie", &cookie);
		}
		Ok(request)
	}

	pub fn from_query_or_filters(
		query: Option<&str>,
		page: i32,
		filters: &[FilterValue],
	) -> Result<Self> {
		let mut category_id = "";
		let mut jindu = "";
		let mut shuxing = "";
		let mut order = "addtime";
		let mut dir = "desc";

		for filter in filters {
			match filter {
				FilterValue::Select { id, value } => match id.as_str() {
					"分类" => category_id = value,
					"进度" => jindu = value,
					"语言" => shuxing = value,
					_ => (),
				},
				FilterValue::Sort {
					id,
					index,
					ascending,
				} => match id.as_str() {
					"排序" => {
						dir = if *ascending { "asc" } else { "desc" };
						match index {
							0 => order = "addtime",
							1 => order = "views",
							2 => order = "favores",
							_ => return Err(error!("Invalid index")),
						}
					}
					_ => return Err(error!("Invalid sort filter id:`{id}`")),
				},

				_ => return Err(error!("Invalid filter:`{filter:?}`")),
			}
		}

		let query = SearchOrFilterQuery::new(query, category_id, jindu, shuxing, order, dir, page);
		Ok(SearchOrFilter(query))
	}
}

impl<'a> Url<'a> {
	pub const fn manga(key: &'a str) -> Self {
		Self::Manga { key }
	}
	pub const fn chapter(key: &'a str) -> Self {
		Self::Chapter { key }
	}
	pub const fn login() -> Self {
		Self::Login
	}
	pub const fn home() -> Self {
		Self::Home
	}
	pub const fn logout(key: &'a str) -> Self {
		Self::Logout { key }
	}
}

pub struct SearchOrFilterQuery(QueryParameters);

impl SearchOrFilterQuery {
	fn new(
		keyword: Option<&str>,
		category_id: &str,
		jindu: &str,
		shuxing: &str,
		order: &str,
		dir: &str,
		page: i32,
	) -> Self {
		let mut q = QueryParameters::new();
		if let Some(keyword) = keyword {
			q.push("keyword", Some(keyword));
		}
		if !category_id.is_empty() {
			q.push_encoded("category_id", Some(category_id));
		}
		if !jindu.is_empty() {
			q.push_encoded("jindu", Some(jindu));
		}
		if !shuxing.is_empty() {
			q.push("shuxing", Some(shuxing));
		}
		q.push_encoded("order", Some(order));
		q.push_encoded("dir", Some(dir));
		q.push_encoded("page", Some(&page.to_string()));
		Self(q)
	}
}

impl Display for SearchOrFilterQuery {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		write!(f, "{}", self.0)
	}
}

fn get_cookie() -> String {
	normalize_cookie(defaults_get::<String>("cookie").unwrap_or_default())
}

fn normalize_cookie(cookie: String) -> String {
	cookie
		.trim()
		.trim_start_matches("Cookie:")
		.trim()
		.replace(",", ";")
		.split(';')
		.filter_map(|part| {
			let part = part.trim();
			let key = part
				.split('=')
				.next()
				.unwrap_or("")
				.trim()
				.to_ascii_lowercase();
			if part.is_empty()
				|| !part.contains('=')
				|| key == "expires"
				|| key == "max-age"
				|| key == "path"
				|| key == "domain"
				|| key == "samesite"
				|| key == "secure"
				|| key == "httponly"
			{
				None
			} else {
				Some(part)
			}
		})
		.collect::<Vec<&str>>()
		.join("; ")
}

fn merge_cookie(first: &str, second: &str) -> String {
	let mut parts: Vec<(String, String)> = Vec::new();
	for cookie in [first, second] {
		for part in cookie.split(';') {
			let part = part.trim();
			let Some((key, value)) = part.split_once('=') else {
				continue;
			};
			let key = key.trim();
			if key.is_empty() {
				continue;
			}
			let value = value.trim();
			if let Some(existing) = parts.iter_mut().find(|(name, _)| name == key) {
				existing.1 = value.to_string();
			} else {
				parts.push((key.to_string(), value.to_string()));
			}
		}
	}
	parts
		.into_iter()
		.map(|(key, value)| format!("{}={}", key, value))
		.collect::<Vec<_>>()
		.join("; ")
}

fn absolute_url(path: &str) -> String {
	if path.starts_with("http://") || path.starts_with("https://") {
		path.to_string()
	} else if path.starts_with('/') {
		format!("{}{}", API_URL, path)
	} else {
		format!("{}/{}", API_URL, path)
	}
}

pub fn login(username: &str, password: &str) -> Result<bool> {
	let home_doc = Url::home().request(HttpMethod::Get)?.html()?;
	if let Some(logout_elem) = home_doc.select_first("a.user-logout-btn")
		&& let Some(logout_href) = logout_elem.attr("href")
	{
		Url::logout(&logout_href).request(HttpMethod::Get)?.send()?;
	}

	let login_response = Url::login().request(HttpMethod::Get)?.send()?;
	let seed_cookie = merge_cookie(
		&get_cookie(),
		&normalize_cookie(login_response.get_header("set-cookie").unwrap_or_default()),
	);
	let login_doc = login_response.get_html()?;

	let formhash = login_doc
		.select_first("input[name='formhash']")
		.ok_or_else(|| error!("formhash not found in form"))?
		.attr("value")
		.ok_or_else(|| error!("No formhash found"))?
		.to_string();

	let form = login_doc
		.select("form[action*='logging&action=login']")
		.ok_or_else(|| error!("formaction not found in form"))?
		.first()
		.ok_or_else(|| error!("No form action found"))?;
	let action = form
		.attr("action")
		.ok_or_else(|| error!("Action not found"))?
		.to_string();

	let post_url = absolute_url(&action);

	let params = [
		("formhash", formhash),
		("referer", format!("{}/./", API_URL)),
		("loginfield", "username".to_string()),
		("username", username.to_string()),
		("password", password.to_string()),
		("cookietime", "2592000".to_string()),
		("loginsubmit", "true".to_string()),
		("questionid", "0".to_string()),
		("answer", "".to_string()),
	];

	let body = params
		.iter()
		.map(|(k, v)| format!("{}={}", k, encode_uri_component(v)))
		.collect::<Vec<_>>()
		.join("&");

	let mut request = Request::new(post_url, HttpMethod::Post)?;
	request.set_header("Content-Type", "application/x-www-form-urlencoded");
	request.set_header("User-Agent", USER_AGENT);
	request.set_header("Referer", API_URL);
	if !seed_cookie.is_empty() {
		request.set_header("Cookie", &seed_cookie);
	}
	request.set_body(body.as_bytes());

	let response = request.send()?;
	let new_cookie = normalize_cookie(response.get_header("set-cookie").unwrap_or_default());
	let final_cookie = merge_cookie(&seed_cookie, &new_cookie);
	if final_cookie.contains("auth") {
		defaults_set("cookie", DefaultValue::String(final_cookie));
		return Ok(true);
	}

	let text = response.get_string()?;
	if text.contains("欢迎您回来") {
		if !final_cookie.is_empty() {
			defaults_set("cookie", DefaultValue::String(final_cookie));
		}
		return Ok(true);
	}
	if text.contains("登录失败") {
		return Ok(false);
	}
	Ok(false)
}
