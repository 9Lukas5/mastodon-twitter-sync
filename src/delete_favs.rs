extern crate chrono;
extern crate egg_mode;
extern crate mammut;
extern crate regex;
extern crate serde_json;
extern crate tokio_core;
extern crate toml;

use chrono::Duration;
use chrono::prelude::*;
use mammut::Mastodon;
use mammut::Error as MammutError;
use std::collections::BTreeMap;
use std::fs::File;
use std::fs::remove_file;
use std::io::prelude::*;

// Delete old favourites of this account that are older than 90 days.
pub fn mastodon_delete_older_favs(mastodon: &Mastodon) {
    // In order not to fetch old favs every time keep them in a cache file
    // keyed by their dates.
    let cache_file = "mastodon_fav_cache.json";
    let dates = mastodon_load_fav_dates(mastodon, cache_file);
    let mut remove_dates = Vec::new();
    let three_months_ago = Utc::now() - Duration::days(90);
    for (date, toot_id) in dates.range(..three_months_ago) {
        println!("Deleting fav {} from {}", toot_id, date);
        remove_dates.push(date);
        // The status could have been deleted already by the user, ignore API
        // errors in that case.
        if let Err(error) = mastodon.unfavourite(*toot_id) {
            match error {
                MammutError::Api(_) => {}
                _ => Err(error).unwrap(),
            }
        }
    }

    let mut new_dates = dates.clone();
    for remove_date in remove_dates {
        new_dates.remove(remove_date);
    }

    if new_dates.is_empty() {
        // If we have deleted all old toots from our cache file we can remove
        // it. On the next run all toots will be fetched and the cache
        // recreated.
        remove_file(cache_file).unwrap();
    } else {
        let json = serde_json::to_string(&new_dates).unwrap();
        let mut file = File::create(cache_file).unwrap();
        file.write_all(json.as_bytes()).unwrap();
    }
}

fn mastodon_load_fav_dates(mastodon: &Mastodon, cache_file: &str) -> BTreeMap<DateTime<Utc>, u64> {
    match load_dates_from_cache(cache_file) {
        Some(dates) => dates,
        None => mastodon_fetch_fav_dates(mastodon, cache_file),
    }
}

fn load_dates_from_cache(cache_file: &str) -> Option<BTreeMap<DateTime<Utc>, u64>> {
    let cache = match File::open(cache_file) {
        Ok(mut file) => {
            let mut json = String::new();
            file.read_to_string(&mut json).unwrap();
            serde_json::from_str(&json).unwrap()
        }
        Err(_) => return None,
    };
    Some(cache)
}

fn mastodon_fetch_fav_dates(mastodon: &Mastodon, cache_file: &str) -> BTreeMap<DateTime<Utc>, u64> {
    let mut dates = BTreeMap::new();
    let mut favourites_pager = mastodon.favourites();
    loop {
        let statuses = favourites_pager.next().unwrap();
        if statuses.is_empty() {
            break;
        }
        for status in statuses {
            dates.insert(status.created_at, status.id);
        }
    }

    let json = serde_json::to_string(&dates).unwrap();
    let mut file = File::create(cache_file).unwrap();
    file.write_all(json.as_bytes()).unwrap();

    dates
}
