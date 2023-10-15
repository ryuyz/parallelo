// #!/usr/bin/env -S cargo +nightly -Zscript
// ```cargo
// [dependencies]
// anyhow = "1.0.75"
// nu-ansi-term = "0.49.0"
// chrono = "0.4.31"
// ```

use std::path::{Path, PathBuf};

use anyhow::Result;

const GCS_ROOT: &str = "gs://fvital-sandbox-bucket/ncchd-asd";

const DOCKER_SHARE: &str = "/root/share";
const DOCKER_YOLO_ROOT: &str = "/root/share/yolov7";
const DOCKER_WEIGHTS: &str = "/root/share/best.pt";
const DOCKER_PROJECT: &str = "/root/share/outs";
const DOCKER_VIDEO_DIRNAME: &str = "/root/inputs";

const HOST_SHARE: &str = "/home/ryutaro_miyata_fvital_tech/yolo/share";
const HOST_PROJECT: &str = "/home/ryutaro_miyata_fvital_tech/yolo/share/outs";

fn main() -> Result<()> {
    for video in list_cleaned_mp4()? {
        let status = fetch_status(&video.stem)?;
        println!("{:?} {}", status, video.stem);
        match status {
            Status::NotYet => {
                do_it(&video);
            }
            _ => {}
        }
    }
    Ok(())
}

/// Returns the absolute path
fn download_mp4(gsuri: &str) -> Result<Abspath> {
    const DIRNAME: &str = "./inputs/";
    std::fs::create_dir_all(DIRNAME).unwrap();

    let abs_dirname = Path::new(DIRNAME).canonicalize().unwrap();
    let abspath = abs_dirname.join(Path::new(gsuri).file_name().unwrap());

    println!("The video will be downloaded to: {}", abspath.display());
    if !std::process::Command::new("gsutil")
        .arg("cp")
        .arg(gsuri)
        .arg(&abs_dirname)
        .status()?
        .success()
    {
        return Err(anyhow::anyhow!("Failed to download"));
    }
    Ok(Abspath::from_host(&abspath))
}

#[derive(Debug)]
struct Abspath {
    host: PathBuf,
    docker: PathBuf,
}

impl Abspath {
    fn from_host(host: &Path) -> Self {
        Self {
            host: host.to_path_buf(),
            docker: Path::new(DOCKER_VIDEO_DIRNAME).join(host.file_name().unwrap()),
        }
    }
}

mod progress {
    use super::*;
    /// Returns the gsuri if successful
    pub(super) fn push(stem: &str) -> Result<String> {
        let gsuri = format!("{}/yolo-outs/inprogress/{}", GCS_ROOT, stem);
        std::process::Command::new("gsutil")
            .arg("cp")
            .arg("-n")
            .arg("/dev/null")
            .arg(&gsuri)
            .output()?
            .status
            .success()
            .then_some(gsuri)
            .ok_or(anyhow::anyhow!("Failed to push stem in progress"))
    }
    pub(super) fn remove(gsuri: &str) -> Result<()> {
        std::process::Command::new("gsutil")
            .arg("rm")
            .arg(gsuri)
            .output()?
            .status
            .success()
            .then_some(())
            .ok_or(anyhow::anyhow!("Failed to remove stem in progress"))
    }
}

fn do_it(video: &Video) {
    let Ok(progress_gsuri) = progress::push(&video.stem) else {
        return eprintln!(
            "{}",
            nu_ansi_term::Color::Red.paint(format!("Failed to push progress {}", video.stem))
        );
    };
    println!(
        "{}",
        nu_ansi_term::Color::Green.paint(format!("{} in progress", video.stem))
    );

    if let Err(err) = download_infer_and_upload(video) {
        eprintln!("{}", nu_ansi_term::Color::Red.paint(format!("{:?}", err)));
    };

    progress::remove(&progress_gsuri).expect("Failed to remove progress");
    println!(
        "{}",
        nu_ansi_term::Color::Green.paint(format!("{} popped from inprogress", video.stem))
    );
}

fn download_infer_and_upload(video: &Video) -> Result<()> {
    println!(
        "{}",
        nu_ansi_term::Color::Green.paint(format!("Downloading... {}", video.stem))
    );
    let abspath = download_mp4(&video.gsuri)?;
    println!(
        "{}",
        nu_ansi_term::Color::Green.paint(format!("Downloaded: {:?}", &abspath))
    );

    let host_outdir = Path::new(HOST_PROJECT).join(&video.stem);
    if host_outdir.exists() {
        let new_dirname = format!("{}_{}", host_outdir.display(), chrono::Utc::now());
        println!(
            "{}",
            nu_ansi_term::Color::Yellow.paint(format!(
                "The output directory already exists. The old one moved to: {:?}",
                &new_dirname
            ))
        );
        fs::sudo_mv(&host_outdir.to_string_lossy(), &new_dirname).unwrap();
        assert!(!host_outdir.exists())
    }

    let device = std::env::var("DEVICE").unwrap_or("0".to_string());

    std::process::Command::new("sudo")
        .args(["bash", "-lc"])
        .arg(format!(
            "docker run --rm --runtime=nvidia --gpus all -it \
            -v '{HOST_SHARE}:{DOCKER_SHARE}' \
            -v '{}:{}' \
            --shm-size=4g yolo \
            python '{DOCKER_YOLO_ROOT}/detect.py' \
            --device '{device}' \
            --weights '{DOCKER_WEIGHTS}' \
            --conf 0.25 \
            --img-size 640 \
            --source '{}' \
            --project '{DOCKER_PROJECT}' \
            --name '{}' \
            --save-txt \
            --save-conf",
            abspath.host.to_string_lossy(),
            abspath.docker.to_string_lossy(),
            abspath.docker.to_string_lossy(),
            video.stem,
        ))
        .status()?;

    std::fs::create_dir_all("./tars").unwrap();
    let tar_abspath = Path::new("./tars")
        .canonicalize()
        .unwrap()
        .join(&format!("{}.tar", video.stem));
    let tar_abspath = tar_abspath.to_string_lossy();
    println!("Archiving to: {}", &tar_abspath);

    std::process::Command::new("tar")
        .args(["cvf", &tar_abspath, "-C", HOST_PROJECT, &video.stem])
        .status()
        .unwrap();

    let gcs_archived_dirname = format!("{}/yolo-outs/archived/", GCS_ROOT);
    std::process::Command::new("gsutil")
        .args(["cp", &tar_abspath, &gcs_archived_dirname])
        .status()?;

    if let Status::Done = fetch_status(&video.stem)? {
        println!(
            "{}",
            nu_ansi_term::Color::Green.paint("Successfully uploaded")
        );
        Ok(())
    } else {
        println!("{}", nu_ansi_term::Color::Red.paint("Failed to upload"));
        Err(anyhow::anyhow!("Failed to upload"))
    }
}

fn fetch_status(stem: &str) -> Result<Status> {
    if std::process::Command::new("gsutil")
        .arg("stat")
        .arg(format!("{}/yolo-outs/archived/{}.tar", GCS_ROOT, stem))
        .output()?
        .status
        .success()
    {
        return Ok(Status::Done);
    }
    if std::process::Command::new("gsutil")
        .arg("stat")
        .arg(format!("{}/yolo-outs/inprogress/{}", GCS_ROOT, stem))
        .output()?
        .status
        .success()
    {
        // TODO
        return Ok(Status::InProgress("".to_string()));
    }
    Ok(Status::NotYet)
}

#[derive(Debug)]
enum Status {
    NotYet,
    InProgress(String),
    Done,
}

fn list_cleaned_mp4() -> Result<Vec<Video>> {
    let cleaned = format!("{}/cleaned", GCS_ROOT);
    let bytes = std::process::Command::new("gsutil")
        .arg("ls")
        .arg(format!("{}/*.mp4", cleaned))
        .output()?
        .stdout;
    Ok(String::from_utf8(bytes)?.lines().map(Video::new).collect())
}

struct Video {
    gsuri: String,
    stem: String,
}

impl Video {
    fn new(gsuri: &str) -> Self {
        let stem = Path::new(gsuri).file_stem().unwrap().to_string_lossy();
        Self {
            gsuri: gsuri.to_string(),
            stem: stem.to_string(),
        }
    }
}

mod fs {
    pub(super) fn sudo_mv(src: &str, dst: &str) -> std::io::Result<()> {
        std::process::Command::new("sudo")
            .args(["mv", src, dst])
            .status()?;
        Ok(())
    }
}
