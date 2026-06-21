use aidoku::{
	error::{AidokuError, AidokuErrorKind, Result},
	helpers::uri::QueryParameters,
	prelude::format,
	std::{
		defaults::{defaults_get, defaults_set},
		html::Node,
		net::{HttpMethod, Request},
		String, StringRef, Vec,
	},
};
use alloc::string::ToString;

pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36";
const DEFAULT_URL: &str = "https://www.zerobyw33.com";

fn default_error() -> AidokuError {
	AidokuError {
		reason: AidokuErrorKind::DefaultNotFound,
	}
}

fn get_default(key: &str) -> String {
	defaults_get(key)
		.and_then(|value| value.as_string())
		.map(|value| value.read())
		.unwrap_or_default()
}

fn trim_url(url: &str) -> String {
	let trimmed = url.trim().trim_end_matches('/');
	if trimmed.is_empty() {
		DEFAULT_URL.to_string()
	} else {
		trimmed.to_string()
	}
}

pub fn get_url() -> String {
	trim_url(&get_default("url"))
}

pub fn get_cookie() -> String {
	get_default("cookie")
		.trim()
		.trim_start_matches("Cookie:")
		.trim()
		.to_string()
}

pub fn referer_url() -> String {
	format!("{}/", get_url())
}

pub fn add_common_headers(request: Request, referer: &str) -> Request {
	let mut request = request
		.header("User-Agent", USER_AGENT)
		.header("Referer", referer);
	let cookie = get_cookie();
	if !cookie.is_empty() {
		request = request.header("Cookie", &cookie);
	}
	request
}

fn gen_request(url: String, method: HttpMethod, referer: &str) -> Request {
	add_common_headers(Request::new(url, method), referer)
}

fn cookie_name(pair: &str) -> &str {
	pair.split("=").next().unwrap_or("").trim()
}

fn push_cookie(cookies: &mut Vec<String>, cookie: &str) {
	let cookie = cookie.trim();
	if cookie.is_empty()
		|| !cookie.contains("=")
		|| cookie.contains("Path=")
		|| cookie.contains("Expires=")
		|| cookie.contains("Max-Age=")
		|| cookie.contains("HttpOnly")
		|| cookie.contains("Secure")
		|| cookie.contains("SameSite=")
	{
		return;
	}

	let name = cookie_name(cookie);
	if name.is_empty() {
		return;
	}

	cookies.retain(|old| cookie_name(old) != name);
	cookies.push(cookie.to_string());
}

fn split_cookie_header(header: &str) -> Vec<String> {
	let mut values = Vec::new();
	let mut current = String::new();
	let mut expires = false;

	for part in header.split(",") {
		let trimmed = part.trim();
		let lower = trimmed.to_ascii_lowercase();
		let starts_cookie = trimmed.contains("=")
			&& !lower.starts_with("expires=")
			&& !lower.starts_with("path=")
			&& !lower.starts_with("max-age=")
			&& !lower.starts_with("domain=")
			&& !lower.starts_with("samesite=");

		if !current.is_empty() && starts_cookie && !expires {
			values.push(current.trim().to_string());
			current = String::new();
		} else if !current.is_empty() {
			current.push_str(",");
		}

		current.push_str(trimmed);
		expires = lower.contains("expires=") && !lower.contains("gmt");
		if lower.contains("gmt") {
			expires = false;
		}
	}

	if !current.is_empty() {
		values.push(current.trim().to_string());
	}

	values
}

fn normalize_cookie_header(cookie_header: &str) -> Vec<String> {
	let mut cookies = Vec::new();
	for set_cookie in split_cookie_header(cookie_header) {
		let value = set_cookie.split(";").next().unwrap_or("").trim();
		push_cookie(&mut cookies, value);
	}
	cookies
}

fn merge_cookies(base_cookie: &str, set_cookie_header: &str) -> String {
	let mut cookies = Vec::new();
	for cookie in base_cookie.trim().trim_start_matches("Cookie:").split(";") {
		push_cookie(&mut cookies, cookie);
	}
	for cookie in normalize_cookie_header(set_cookie_header) {
		push_cookie(&mut cookies, &cookie);
	}
	cookies.join("; ")
}

fn page_requires_login(html: &Node) -> bool {
	let restricted_text = html.select("#main_message #messagetext").text().read();
	if restricted_text.trim().is_empty() {
		return false;
	}

	restricted_text.contains("login")
		|| restricted_text.contains("Login")
		|| restricted_text.contains("permission")
		|| restricted_text.contains("access denied")
		|| restricted_text.contains("requires login")
}

fn login(form_html: &Node, current_cookie: &str) -> Result<String> {
	let username = get_default("username");
	let password = get_default("password");
	if username.trim().is_empty() || password.is_empty() {
		return Err(default_error());
	}

	let formhash = form_html
		.select("input[name=formhash]")
		.attr("value")
		.read();
	let loginhash = form_html
		.select("input[name=loginhash]")
		.attr("value")
		.read();
	let mut params = QueryParameters::new();
	params.push("username", Some(username.trim()));
	params.push("cookietime", Some("2592000"));
	params.push("password", Some(&password));
	if !formhash.is_empty() {
		params.push("formhash", Some(&formhash));
	}
	params.push("quickforward", Some("yes"));
	params.push("handlekey", Some("ls"));
	params.push("loginsubmit", Some("yes"));

	let extra = if loginhash.is_empty() {
		String::new()
	} else {
		format!("&loginhash={loginhash}")
	};
	let login_url = format!(
		"{}/member.php?mod=logging&action=login&infloat=yes&lssubmit=yes&inajax=1{}",
		get_url(),
		extra
	);
	let body = params.to_string();
	let request = gen_request(login_url, HttpMethod::Post, &referer_url())
		.header("Content-Type", "application/x-www-form-urlencoded")
		.header("Cookie", current_cookie)
		.body(body.as_bytes());

	request.send();

	let set_cookie = request.get_header("set-cookie").unwrap_or_default().read();
	let merged_cookie = merge_cookies(current_cookie, &set_cookie);
	if merged_cookie == current_cookie || !merged_cookie.contains("=") {
		return Err(default_error());
	}

	defaults_set("cookie", StringRef::from(merged_cookie.clone()).0);
	Ok(merged_cookie)
}

fn fetch_html(url: &str, cookie: &str) -> Result<Node> {
	let mut request = gen_request(url.to_string(), HttpMethod::Get, &referer_url());
	if !cookie.is_empty() {
		request = request.header("Cookie", cookie);
	}
	request.send();

	let set_cookie = request.get_header("set-cookie").unwrap_or_default().read();
	if !set_cookie.is_empty() {
		let merged = merge_cookies(cookie, &set_cookie);
		if merged != cookie {
			defaults_set("cookie", StringRef::from(merged).0);
		}
	}

	request.html()
}

pub fn get_html(url: String) -> Result<Node> {
	get_html_inner(url, true)
}

fn get_html_inner(url: String, allow_login_retry: bool) -> Result<Node> {
	let cookie = get_cookie();
	let html = fetch_html(&url, &cookie)?;

	if !page_requires_login(&html) {
		return Ok(html);
	}

	if !allow_login_retry {
		return Err(default_error());
	}

	let new_cookie = login(&html, &cookie)?;
	let html = fetch_html(&url, &new_cookie)?;
	if page_requires_login(&html) {
		return Err(default_error());
	}

	Ok(html)
}
