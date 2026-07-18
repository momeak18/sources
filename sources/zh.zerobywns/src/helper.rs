use aidoku::{
	prelude::format,
	std::{defaults::defaults_get, html::Node, net::Request, String},
};
use alloc::string::ToString;

pub const BASE_URL: &str = "https://www.zerobyw33.com";
pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0.1 Safari/605.1.15";

pub fn get_url() -> String {
	defaults_get("url")
		.and_then(|value| value.as_string())
		.map(|value| value.read())
		.unwrap_or_else(|_| BASE_URL.to_string())
		.trim()
		.trim_end_matches('/')
		.to_string()
}

pub fn get_cookie() -> String {
	defaults_get("cookie")
		.and_then(|value| value.as_string())
		.map(|value| value.read())
		.unwrap_or_default()
		.trim()
		.trim_start_matches("Cookie:")
		.trim()
		.to_string()
}

pub fn request(url: String) -> aidoku::error::Result<Request> {
	let referer = format!("{}/", get_url());
	let mut request = Request::get(url)
		.header("User-Agent", USER_AGENT)
		.header("Referer", &referer);
	let cookie = get_cookie();
	if !cookie.is_empty() {
		request = request.header("Cookie", &cookie);
	}
	Ok(request)
}

pub fn html(url: String) -> aidoku::error::Result<Node> {
	request(url)?.html()
}

pub fn absolute_url(path: &str) -> String {
	if path.starts_with("http://") || path.starts_with("https://") {
		path.to_string()
	} else if path.starts_with("//") {
		format!("https:{path}")
	} else if path.starts_with('/') {
		format!("{}{}", get_url(), path)
	} else {
		format!("{}/{}", get_url(), path.trim_start_matches("./"))
	}
}
