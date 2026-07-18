use aidoku::{
	error::{AidokuError, AidokuErrorKind, Result},
	prelude::format,
	std::{
		defaults::{defaults_get, defaults_set},
		html::Node,
		net::{HttpMethod, Request},
		String, StringRef, Vec,
	},
};
use alloc::string::ToString;

pub const BASE_URL: &str = "https://www.zerobyw33.com";
pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0.1 Safari/605.1.15";

fn get_default(key: &str) -> String {
	defaults_get(key)
		.and_then(|value| value.as_string())
		.map(|value| value.read())
		.unwrap_or_default()
		.trim()
		.to_string()
}

pub fn get_url() -> String {
	let url = get_default("url");
	let url = if url.is_empty() {
		BASE_URL.to_string()
	} else {
		url
	};
	url.trim().trim_end_matches('/').to_string()
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
			{
				None
			} else {
				Some(part)
			}
		})
		.collect::<Vec<&str>>()
		.join("; ")
}

pub fn get_cookie() -> String {
	normalize_cookie(get_default("cookie"))
}

fn apply_headers(request: Request, cookie: &str) -> Request {
	let referer = format!("{}/", get_url());
	let mut request = request
		.header("User-Agent", USER_AGENT)
		.header("Referer", &referer);
	if !cookie.is_empty() {
		request = request.header("Cookie", cookie);
	}
	request
}

fn is_login_required(html: &Node) -> bool {
	html.select("#main_message #messagetext>p")
		.text()
		.read()
		.contains("only registered members can view")
		|| html
			.select("#main_message #messagetext>p")
			.text()
			.read()
			.contains("only for logged-in users")
		|| html
			.select("#main_message #messagetext>p")
			.text()
			.read()
			.contains("login")
		|| html
			.select("#main_message #messagetext>p")
			.text()
			.read()
			.contains("\u{4ec5}\u{9650}\u{7528}\u{6237}\u{89c2}\u{770b}")
}

fn login() -> Result<String> {
	let username = get_default("username");
	let password = get_default("password");
	if username.is_empty() || password.is_empty() {
		return Err(AidokuError {
			reason: AidokuErrorKind::DefaultNotFound,
		});
	}

	let login_page_url = format!("{}/member.php?mod=logging&action=login", get_url());
	let login_page_request = apply_headers(Request::get(login_page_url), &get_cookie());
	login_page_request.send();
	let login_seed_cookie = normalize_cookie(
		login_page_request
			.get_header("set-cookie")
			.unwrap_or_default()
			.read(),
	);
	let login_page = login_page_request.html()?;
	let formhash = login_page
		.select("input[name=formhash]")
		.attr("value")
		.read();
	let body = format!(
		"username={}&cookietime=2592000&password={}&formhash={}&quickforward=yes&handlekey=ls",
		username, password, formhash
	);
	let login_url = format!(
		"{}/member.php?mod=logging&action=login&loginsubmit=yes&infloat=yes&lssubmit=yes&inajax=1",
		get_url()
	);
	let login_request = Request::new(login_url, HttpMethod::Post)
		.header("User-Agent", USER_AGENT)
		.header("Referer", &format!("{}/", get_url()))
		.header("Content-Type", "application/x-www-form-urlencoded")
		.header("Cookie", &login_seed_cookie)
		.body(body.as_bytes());

	login_request.send();
	let new_cookie = normalize_cookie(
		login_request
			.get_header("set-cookie")
			.unwrap_or_default()
			.read(),
	);

	if !new_cookie.contains("auth") {
		return Err(AidokuError {
			reason: AidokuErrorKind::DefaultNotFound,
		});
	}

	defaults_set("cookie", StringRef::from(new_cookie.clone()).0);
	Ok(new_cookie)
}

pub fn html(url: String) -> Result<Node> {
	let request = apply_headers(Request::get(url.clone()), &get_cookie());
	request.send();
	let html = request.html()?;
	if is_login_required(&html) {
		let cookie = login()?;
		let request = apply_headers(Request::get(url), &cookie);
		return request.html();
	}
	Ok(html)
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
