use crate::common::*;
use async_std::{
    fs::{self, File},
    io::BufWriter,
    task::sleep,
};
use serde_loader::AbsPathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Config {
    pub dir: AbsPathBuf,
    #[serde(default)]
    pub auto_clean: bool,
}

impl Config {
    pub async fn build_sender(&self) -> Result<Sender> {
        let Self {
            ref dir,
            auto_clean,
        } = *self;

        if auto_clean {
            let result = fs::remove_dir_all(dir.get()).await;
            if let Err(err) = result {
                if err.kind() != io::ErrorKind::NotFound {
                    return Err(err.into());
                }
            }
        }

        fs::create_dir_all(dir.get()).await?;

        Ok(Sender { dir: dir.clone() })
    }

    pub async fn build_receiver(&self) -> Result<Receiver> {
        let dir = &*self.dir;

        // Read the directory. If the dir doesn't exist, wait a moment and retry,
        let readdir = loop {
            match fs::read_dir(dir).await {
                Ok(readdir) => break readdir,
                Err(err) => {
                    if err.kind() == io::ErrorKind::NotFound {
                        sleep(Duration::from_millis(500)).await;
                    } else {
                        return Err(err.into());
                    }
                }
            }
        };

        let mut files: Vec<_> = readdir
            .try_filter_map(|entry| async move {
                if entry.file_type().await?.is_dir() {
                    return Ok(None);
                }
                let path = entry.path();
                let dt = (move || -> Option<_> {
                    let file_name = entry.file_name();
                    let file_name = file_name.to_str()?;
                    let dt = DateTime::parse_from_rfc3339(file_name).ok()?;
                    Some(dt)
                })();
                let path: PathBuf = path.into_os_string().into();

                Ok(Some((dt, path)))
            })
            .try_collect()
            .await?;

        let files: Vec<_> = {
            files.sort_by_cached_key(|(dt, _)| *dt);
            files.into_iter().map(|(_, path)| path).collect()
        };

        Ok(Receiver { index: 0, files })
    }
}

#[derive(Debug)]
pub struct Sender {
    dir: AbsPathBuf,
}

impl Sender {
    pub async fn send(&self, payload: &[u8]) -> Result<()> {
        let file_name = Local::now().to_rfc3339_opts(SecondsFormat::Nanos, false);
        let path = self.dir.join(&file_name);
        let mut file = BufWriter::new(File::create(path).await?);
        file.write_all(payload).await?;
        file.flush().await?;
        Ok(())
    }

    pub fn into_sink(self) -> impl Sink<Vec<u8>, Error = Error> {
        sink::unfold(self, |sender, payload: Vec<u8>| async move {
            sender.send(&payload).await.map(|_| sender)
        })
    }
}

#[derive(Debug)]
pub struct Receiver {
    index: usize,
    files: Vec<PathBuf>,
}

impl Receiver {
    pub async fn recv(&mut self) -> Result<Option<Vec<u8>>> {
        let path = match self.files.get(self.index) {
            Some(path) => path,
            None => return Ok(None),
        };
        self.index += 1;

        let mut file = fs::File::open(path).await?;
        let mut payload = Vec::<u8>::new();
        file.read_to_end(&mut payload).await?;

        Ok(Some(payload))
    }

    pub fn into_stream(self) -> impl Stream<Item = Result<Vec<u8>>> {
        stream::try_unfold(self, |mut rx| async move {
            let item = rx.recv().await?;
            anyhow::Ok(item.map(|item| (item, rx)))
        })
    }
}
