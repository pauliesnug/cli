use crate::{
	config::Config,
	fatal,
	index::{self, AdminAction},
	info,
	logging::ask_value,
	server::{ApiResponse, PaginatedData},
	warn, NiceUnwrap,
};

use rand::Rng;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
struct PendingMod {
	id: String,
	repository: Option<String>,
	versions: Vec<PendingModVersion>,
	tags: Vec<String>,
	about: Option<String>,
	changelog: Option<String>,
}

impl PendingMod {
	fn print(&self) {
		println!("{}", self.id);
		println!(
			"- Repository: {}",
			self.repository.as_deref().unwrap_or("None")
		);
		println!("- Tags: {}", self.tags.join(", "));
		println!("- About");
		println!("----------------------------");
		println!("{}", self.about.as_deref().unwrap_or("None"));
		println!("----------------------------");
		// To be honest I have no idea if we should show this, it can become quite large
		// println!("- Changelog");
		// println!("----------------------------");
		// println!("{}", self.changelog.as_deref().unwrap_or("None"));
		// println!("----------------------------");
		println!("- Versions:");
		for (i, version) in self.versions.iter().enumerate() {
			version.print(i + 1);
		}
	}
}

#[derive(Debug, Deserialize, Clone)]
struct PendingModVersion {
	name: String,
	version: String,
	description: Option<String>,
	geode: String,
	early_load: bool,
	api: bool,
	gd: PendingModGD,
	dependencies: Option<Vec<PendingModDepencency>>,
	incompatibilities: Option<Vec<PendingModDepencency>>,
}

impl PendingModVersion {
	fn print(&self, index: usize) {
		println!("{}. {}", index, self.version);
		println!("  - Name: {}", self.name);
		println!(
			"  - Description: {}",
			self.description.as_deref().unwrap_or("None")
		);
		println!("  - Geode: {}", self.geode);
		println!("  - Early Load: {}", self.early_load);
		println!("  - API: {}", self.api);
		println!("  - GD:");
		println!("    - Win: {}", self.gd.win.as_deref().unwrap_or("None"));
		println!("    - Mac: {}", self.gd.mac.as_deref().unwrap_or("None"));
		println!(
			"    - Android 32: {}",
			self.gd.android32.as_deref().unwrap_or("None")
		);
		println!(
			"    - Android 64: {}",
			self.gd.android64.as_deref().unwrap_or("None")
		);
		println!("    - iOS: {}", self.gd.ios.as_deref().unwrap_or("None"));
		if let Some(deps) = &self.dependencies {
			println!("  - Dependencies:");
			for (i, dep) in deps.iter().enumerate() {
				dep.print(i + 1);
			}
		}
		if let Some(incomps) = &self.incompatibilities {
			println!("  - Incompatibilities:");
			for (i, incomp) in incomps.iter().enumerate() {
				incomp.print(i + 1);
			}
		}
	}
}

#[derive(Debug, Deserialize, Clone)]
struct PendingModGD {
	win: Option<String>,
	mac: Option<String>,
	android32: Option<String>,
	android64: Option<String>,
	ios: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct PendingModDepencency {
	mod_id: String,
	version: String,
	importance: String,
}

impl PendingModDepencency {
	fn print(&self, index: usize) {
		println!("    {}. {}", index, self.mod_id);
		println!("      - Version: {}", self.version);
		println!("      - Importance: {}", self.importance);
	}
}

pub fn admin_dashboard(action: AdminAction, config: &mut Config) {
	if config.index_token.is_none() {
		fatal!("You are not logged in!");
	}
	// let profile = index::get_user_profile(config);
	// if !profile.admin {
	// 	let message = get_random_message();
	// 	fatal!("{}", message);
	// }

	match action {
		AdminAction::ListPending => {
			list_pending_mods(config);
		}
		_ => unimplemented!(),
	}
}

fn get_pending_mods(page: i32, config: &Config) -> PaginatedData<PendingMod> {
	if config.index_token.is_none() {
		fatal!("You are not logged in!");
	}

	let client = reqwest::blocking::Client::new();
	let path = format!("v1/mods?pending_validation=true&page={}&per_page=1", page);
	let url = index::get_index_url(path, config);

	let response = client
		.get(url)
		.bearer_auth(config.index_token.clone().unwrap())
		.send()
		.nice_unwrap("Failed to connect to the Geode Index");

	if response.status() != 200 {
		if let Ok(body) = response.json::<ApiResponse<String>>() {
			warn!("{}", body.error);
		}
		fatal!("Bad response from Geode Index");
	}

	let data: ApiResponse<PaginatedData<PendingMod>> = response
		.json()
		.nice_unwrap("Failed to parse response from the Geode Index");

	data.payload
}

fn list_pending_mods(config: &Config) {
	let mut page = 1;

	loop {
		let mods = get_pending_mods(page, config);
		info!("{:?}", mods);

		if mods.count == 0 {
			info!("No pending mods on the index");
			break;
		}

		print!("{esc}c", esc = 27 as char);

		for entry in mods.data.iter() {
			entry.print();
		}

		println!("---------------------");
		println!("Submission {}/{}", page, mods.count);
		println!("---------------------");
		println!("Commands:");
		println!("  - n: Next submission");
		println!("  - p: Previous submission");
		println!("  - <INDEX>: Go to submission");
		println!("  - v: Validate mod");
		println!("  - r: Reject mod");
		println!("  - q: Quit");
		println!("---------------------");

		let choice = ask_value("Action", None, true);

		match choice.trim() {
			"n" => {
				if page < mods.count {
					page += 1;
				}
			}
			"p" => {
				if page > 1 {
					page -= 1;
				}
			}
			"v" => {
				let version_vec: &Vec<PendingModVersion> = mods.data[0].versions.as_ref();

				if version_vec.len() == 1 {
					validate_mod(&version_vec[0], &mods.data[0].id, config);
				} else {
					let version = ask_value("Version", None, true);
					if let Some(version) = version_vec.iter().find(|x| x.version == version) {
						validate_mod(version, &mods.data[0].id, config);
					} else {
						warn!("Invalid version");
					}
				}
			}
			"r" => {
				// reject_mod();
			}
			"q" => {
				break;
			}
			_ => {
				if let Ok(new_page) = choice.parse::<i32>() {
					if new_page < 1 || new_page > mods.count {
						warn!("Invalid page number");
					} else {
						page = new_page;
					}
				} else {
					warn!("Invalid input");
				}
			}
		}
	}
}

fn validate_mod(version: &PendingModVersion, id: &str, config: &Config) {
	if config.index_token.is_none() {
		fatal!("You are not logged in!");
	}
	let client = reqwest::blocking::Client::new();
	let path = format!("v1/mods/{}/versions/{}", id, version.version);
	let url = index::get_index_url(path, config);

	let response = client
		.put(url)
		.bearer_auth(config.index_token.clone().unwrap())
		.json(&json!({
			"validated": true
		}))
		.send()
		.nice_unwrap("Failed to connect to the Geode Index");

	if response.status() != 204 {
		if let Ok(body) = response.json::<ApiResponse<String>>() {
			warn!("{}", body.error);
		}
		fatal!("Bad response from Geode Index");
	}

	info!("Mod validated");
}

pub fn get_random_message() -> String {
	let messages = [
		"[BUZZER]",
		"Your princess is in another castle",
		"Absolutely not",
		"Get lost",
		"Sucks to be you",
		"No admin, laugh at this user",
		"Admin dashboard",
		"Why are we here? Just to suffer?",
		"You hacked the mainframe! Congrats.",
		"You're an admin, Harry",
	];

	let mut rng = rand::thread_rng();
	let index = rng.gen_range(0..messages.len());
	messages[index].to_string()
}
