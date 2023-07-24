use std::{
    collections::HashMap,
    env,
    fmt::Display,
    fs,
    io::{self, Write},
    ops::Range,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use bytes::Bytes;
use rand::{thread_rng, Rng};
use reqwest::IntoUrl;
use serde::{Deserialize, Serialize};

pub mod cli;

#[derive(Debug)]
pub enum Error {
    IOError(std::io::Error),
    SerdeError(serde_json::Error),
    ReqwestError(reqwest::Error),
    Other(String),
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::IOError(value)
    }
}
impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::SerdeError(value)
    }
}
impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        Self::ReqwestError(value)
    }
}
impl From<String> for Error {
    fn from(value: String) -> Self {
        Self::Other(value)
    }
}

#[derive(Serialize, Deserialize)]
pub struct CachedFile {
    pub url: String,
    pub path: String,
    pub cached_time: u64, //UNIX_APOCH time
    pub random: i32,      //random offset in cache lifetime
}

pub struct Config {
    pub cache_json: PathBuf,
    pub cache_dir: PathBuf,
    pub default_offset_range: Range<i32>,
    pub file_lifetime: u64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            cache_json: PathBuf::from(get_absolute_path_with_variables(
                "$HOME/.config/file-cacher/cache.json",
            )),
            cache_dir: PathBuf::from(get_absolute_path_with_variables(
                "$HOME/.config/file-cacher/cache/",
            )),
            default_offset_range: -36000..36001,
            file_lifetime: 1728000,
        }
    }
}

pub struct Cacher {
    config: Config,
    file_map: HashMap<String, CachedFile>,
}

impl Cacher {
    pub fn new(config: Option<Config>) -> Cacher {
        let config = config.unwrap_or_default();
        let map = Cacher::load(&config.cache_json);
        Cacher {
            config,
            file_map: map,
        }
    }

    pub fn stats(&self) -> Stats {
        let mut number_of_cached_files: i32 = 0;
        let mut max_file_size: (String, u64) = ("".to_string(), 0);
        let mut min_file_size: (String, u64) = ("".to_string(), u64::MAX);
        let mut total_size: u64 = 0;
        let mut last_to_expire: (String, u64) = ("".to_string(), 0);
        let mut first_to_expire: (String, u64) = ("".to_string(), u64::MAX);

        for cached_file in self.file_map.values() {
            if let Ok(metadata) = fs::metadata(&cached_file.path) {
                if metadata.is_file() {
                    number_of_cached_files += 1;
                    if metadata.len() > max_file_size.1 {
                        max_file_size = (cached_file.path.clone(), metadata.len());
                    }
                    if metadata.len() < min_file_size.1 {
                        min_file_size = (cached_file.path.clone(), metadata.len());
                    }
                    total_size += metadata.len();
                    let expire_time =
                        Cacher::get_expire_time(cached_file, self.config.file_lifetime);
                    if expire_time > last_to_expire.1 {
                        last_to_expire = (cached_file.path.clone(), expire_time);
                    }
                    if expire_time < first_to_expire.1 {
                        first_to_expire = (cached_file.path.clone(), expire_time);
                    }
                }
            }
        }

        Stats {
            first_to_expire,
            number_of_cached_files,
            max_file_size,
            min_file_size,
            total_size,
            last_to_expire,
        }
    }

    pub fn save(&self) -> Result<(), Error> {
        let text = serde_json::to_string_pretty(&self.file_map)?;
        fs::write(&self.config.cache_json, text)?;
        Ok(())
    }

    fn load<P: AsRef<Path>>(path: P) -> HashMap<String, CachedFile> {
        if let Ok(text) = fs::read_to_string(path) {
            let map: Result<HashMap<String, CachedFile>, serde_json::Error> =
                serde_json::from_str(&text);
            if let Ok(file_map) = map {
                return file_map;
            }
        }
        HashMap::new()
    }

    fn init_path(&self) -> Result<(), Error> {
        fs::create_dir_all(&self.config.cache_dir)?;
        Ok(())
    }

    pub fn clean_expired(&mut self) -> u16 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();
        let mut to_remove: Vec<String> = Vec::new();
        for key in self.file_map.keys() {
            let cached_file = self.file_map.get(key).unwrap();
            let expire_time = Cacher::get_expire_time(cached_file, self.config.file_lifetime);
            if expire_time < now {
                to_remove.push(key.clone());
            }
        }
        let mut count = 0;
        for key in to_remove {
            let cached_file = self.file_map.remove(&key).unwrap();
            if let Err(err) = fs::remove_file(&cached_file.path) {
                writeln!(io::stderr(), "{},{}", err, cached_file.path)
                    .expect("printing to stderr failed");
            }
            count+=1;
        }
        count
    }

    fn get_expire_time(cached_file: &CachedFile, file_lifetime: u64) -> u64 {
        match cached_file.random < 0 {
            true => {
                cached_file.cached_time + file_lifetime - cached_file.random.unsigned_abs() as u64
            }
            false => cached_file.cached_time + file_lifetime + cached_file.random as u64,
        }
    }

    pub async fn get<T: IntoUrl>(
        &mut self,
        url: T,
        filename: &str,
        refresh: bool,
    ) -> Result<String, Error> {
        self.clean_expired();
        if !refresh {
            if let Some(cached_file) = self.file_map.get(url.as_str()) {
                let path = PathBuf::from(&cached_file.path);
                if path.exists() && path.is_file() {
                    return Ok(cached_file.path.clone());
                }
            }
        }
        self.file_map.remove(url.as_str());

        let bytes = Cacher::get_from_url(url.as_str()).await?;
        let mut counter = 0;
        let filenames: Vec<String> = self
            .file_map
            .values()
            .map(|cached_file| {
                Path::new(&cached_file.path)
                    .file_name()
                    .expect("corrupted cache")
                    .to_str()
                    .expect("path wasn't valid UTF-8")
                    .to_string()
            })
            .collect();
        let mut filename_test = filename.to_owned();
        while filenames.contains(&filename_test) {
            counter += 1;
            filename_test = counter.to_string() + filename;
        }

        let mut path = self.config.cache_dir.clone();
        path.push(filename_test);
        self.init_path()?;
        fs::write(&path, bytes)?;
        let path = path.to_str().expect("path wasn't valid UTF-8").to_owned();
        let mut rng = thread_rng();
        let cached_file = CachedFile {
            path,
            url: url.as_str().to_owned(),
            cached_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs(),
            random: rng.gen_range(self.config.default_offset_range.clone()),
        };
        self.file_map.insert(url.as_str().to_owned(), cached_file);

        return Ok(self.file_map.get(url.as_str()).unwrap().path.to_owned());
    }

    async fn get_from_url<T: IntoUrl>(url: T) -> reqwest::Result<Bytes> {
        reqwest::get(url).await?.bytes().await
    }

    pub fn clear(&mut self) -> Result<i16, Error> {
        let mut count=0;
        for path in self.file_map.values().map(|v| &v.path) {
            if let Err(err) = fs::remove_file(path) {
                writeln!(io::stderr(), "{},{}", err, path).expect("printing to stderr failed");
            }
            count+=1;
        }
        self.file_map.clear();
        self.save()?;
        Ok(count)
    }
}

pub struct Stats {
    pub number_of_cached_files: i32,
    pub max_file_size: (String, u64),
    pub min_file_size: (String, u64),
    pub total_size: u64,
    pub first_to_expire: (String, u64),
    pub last_to_expire: (String, u64),
}

impl Display for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut buf = String::new();
        buf.push_str(&format!(
            "number of files in cache: {}\n",
            self.number_of_cached_files
        ));
        buf.push_str(&format!(
            "largest file: {}\n\tsize:{} bytes\n",
            self.max_file_size.0, self.max_file_size.1
        ));
        buf.push_str(&format!(
            "smallest file: {}\n\tsize:{} bytes\n",
            self.min_file_size.0, self.min_file_size.1
        ));
        buf.push_str(&format!("total cache size: {} bytes\n", self.total_size));
        buf.push_str(&format!(
            "next file to expire: {}\n\t{}\n",
            self.first_to_expire.0,
            format_unix_epoch_duration(&self.first_to_expire.1)
        ));
        buf.push_str(&format!(
            "last file to expire: {}\n\t{}\n",
            self.last_to_expire.0,
            format_unix_epoch_duration(&self.last_to_expire.1)
        ));
        write!(f, "{}", buf)
    }
}

fn format_unix_epoch_duration(time: &u64) -> String {
    let time: i64 = *time as i64
        - SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as i64;
    if time < 0 {
        format!("{}s ago", time)
    } else {
        format!("in {}s", time)
    }
}

pub fn get_absolute_path_with_variables(path: &str) -> String {
    let mut final_path: PathBuf = PathBuf::new();
    let absolute_path = PathBuf::from(path.clone())
        .canonicalize()
        .unwrap_or(PathBuf::from(path.clone()));
    for component in absolute_path.components() {
        let component = component.as_os_str().to_str().unwrap();
        if let Some(var) = component.strip_prefix('$') {
            let var = env::var(var).unwrap_or("".to_string());
            for component in PathBuf::from(var).components() {
                if let Component::RootDir = component {
                    if final_path.components().count() != 0 {
                        continue;
                    }
                }
                final_path.push(component);
            }
        } else {
            final_path.push(component);
        }
    }

    final_path.to_str().unwrap_or(path).to_string()
}
