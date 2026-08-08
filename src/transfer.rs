use crate::{
    cli::Repo,
    error::{Error, Result},
    hub::{Hub, status_error},
    plan::PlannedFile,
};
use futures_util::{StreamExt, stream};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::{
    StatusCode,
    header::{CONTENT_LENGTH, CONTENT_RANGE, RANGE},
};
use std::sync::Arc;
use tokio::{
    fs::{self, File, OpenOptions},
    io::AsyncWriteExt,
};

pub async fn download_all(
    hub: Hub,
    repo: &Repo,
    revision: &str,
    plan: Vec<PlannedFile>,
    jobs: usize,
    max_parallel_parts_per_job: usize,
    quiet: bool,
) -> Result<()> {
    let bars = Arc::new(MultiProgress::new());
    stream::iter(plan)
        .map(|file| {
            let hub = hub.clone();
            let repo = repo.clone();
            let revision = revision.to_owned();
            let bars = bars.clone();
            async move {
                download_one(
                    &hub,
                    &repo,
                    &revision,
                    file,
                    bars,
                    max_parallel_parts_per_job,
                    quiet,
                )
                .await
            }
        })
        .buffer_unordered(jobs)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect()
}
async fn download_one(
    hub: &Hub,
    repo: &Repo,
    revision: &str,
    file: PlannedFile,
    bars: Arc<MultiProgress>,
    max_parallel_parts: usize,
    quiet: bool,
) -> Result<()> {
    if let Ok(meta) = fs::metadata(&file.destination).await
        && meta.len() == file.size
    {
        return Ok(());
    }
    if let Some(parent) = file.destination.parent() {
        fs::create_dir_all(parent).await?;
    }
    let part = file.destination.with_file_name(format!(
        "{}.part",
        file.destination.file_name().unwrap().to_string_lossy()
    ));
    let mut offset = fs::metadata(&part).await.map(|m| m.len()).unwrap_or(0);
    if offset > file.size {
        fs::remove_file(&part).await?;
        offset = 0;
    }
    let url = hub.file_url(repo, revision, &file.remote)?;
    let bar = progress_bar(&bars, &file, offset, quiet);
    if offset == 0 && max_parallel_parts > 1 && file.size > 1 {
        download_parts(
            hub,
            url,
            &file.remote,
            file.size,
            &part,
            max_parallel_parts,
            &bar,
        )
        .await?;
        bar.finish_and_clear();
        fs::rename(part, file.destination).await?;
        return Ok(());
    }
    let mut request = hub.get(url);
    if offset > 0 {
        request = request.header(RANGE, format!("bytes={offset}-"));
    }
    let response = request.send().await?;
    let status = response.status();
    if offset > 0 {
        if status != StatusCode::PARTIAL_CONTENT
            || !valid_range(
                response
                    .headers()
                    .get(CONTENT_RANGE)
                    .and_then(|h| h.to_str().ok()),
                offset,
            )
        {
            return Err(Error::InvalidRange(file.remote));
        }
    } else if !status.is_success() {
        return Err(status_error(
            status,
            &file.remote,
            response.text().await.unwrap_or_default(),
        ));
    }
    if offset == 0 && status != StatusCode::OK && status != StatusCode::PARTIAL_CONTENT {
        return Err(status_error(
            status,
            &file.remote,
            response.text().await.unwrap_or_default(),
        ));
    }
    if let Some(length) = response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|h| h.to_str().ok())
        .and_then(|x| x.parse::<u64>().ok())
        && offset + length != file.size
    {
        return Err(Error::SizeMismatch {
            path: file.remote,
            expected: file.size,
            actual: offset + length,
        });
    }
    let mut output = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&part)
        .await?;
    let mut body = response.bytes_stream();
    while let Some(chunk) = body.next().await {
        let chunk = chunk?;
        output.write_all(&chunk).await?;
        offset += chunk.len() as u64;
        bar.inc(chunk.len() as u64);
    }
    output.flush().await?;
    bar.finish_and_clear();
    if offset != file.size {
        return Err(Error::SizeMismatch {
            path: file.remote,
            expected: file.size,
            actual: offset,
        });
    }
    fs::rename(part, file.destination).await?;
    Ok(())
}
fn progress_bar(bars: &MultiProgress, file: &PlannedFile, offset: u64, quiet: bool) -> ProgressBar {
    if quiet {
        ProgressBar::hidden()
    } else {
        let bar = bars.add(ProgressBar::new(file.size));
        bar_style(&bar);
        bar.set_message(file.remote.clone());
        bar.set_position(offset);
        bar
    }
}

async fn download_parts(
    hub: &Hub,
    url: reqwest::Url,
    remote: &str,
    size: u64,
    destination: &std::path::Path,
    max_parts: usize,
    bar: &ProgressBar,
) -> Result<()> {
    let ranges = split_ranges(size, max_parts);
    stream::iter(ranges.iter().copied().enumerate())
        .map(|(index, (start, end))| {
            let hub = hub.clone();
            let url = url.clone();
            let remote = remote.to_owned();
            let piece = destination.with_file_name(format!(
                "{}.part.{index}",
                destination.file_name().unwrap().to_string_lossy()
            ));
            let bar = bar.clone();
            async move { download_range(&hub, url, &remote, start, end, &piece, &bar).await }
        })
        .buffer_unordered(max_parts.min(ranges.len()))
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>>>()?;

    let mut output = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(destination)
        .await?;
    for (index, &(start, end)) in ranges.iter().enumerate() {
        let piece = destination.with_file_name(format!(
            "{}.part.{index}",
            destination.file_name().unwrap().to_string_lossy()
        ));
        let mut input = File::open(&piece).await?;
        let copied = tokio::io::copy(&mut input, &mut output).await?;
        if copied != end - start + 1 {
            return Err(Error::SizeMismatch {
                path: remote.into(),
                expected: end - start + 1,
                actual: copied,
            });
        }
        fs::remove_file(piece).await?;
    }
    output.flush().await?;
    let actual = fs::metadata(destination).await?.len();
    if actual != size {
        return Err(Error::SizeMismatch {
            path: remote.into(),
            expected: size,
            actual,
        });
    }
    Ok(())
}

async fn download_range(
    hub: &Hub,
    url: reqwest::Url,
    remote: &str,
    start: u64,
    end: u64,
    piece: &std::path::Path,
    bar: &ProgressBar,
) -> Result<()> {
    let response = hub
        .get(url)
        .header(RANGE, format!("bytes={start}-{end}"))
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(status_error(
            response.status(),
            remote,
            response.text().await.unwrap_or_default(),
        ));
    }
    if response.status() != StatusCode::PARTIAL_CONTENT
        || !valid_range(
            response
                .headers()
                .get(CONTENT_RANGE)
                .and_then(|h| h.to_str().ok()),
            start,
        )
    {
        return Err(Error::InvalidRange(remote.into()));
    }
    let expected = end - start + 1;
    if response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|h| h.to_str().ok())
        .and_then(|x| x.parse::<u64>().ok())
        != Some(expected)
    {
        return Err(Error::SizeMismatch {
            path: remote.into(),
            expected,
            actual: 0,
        });
    }
    let mut output = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(piece)
        .await?;
    let mut body = response.bytes_stream();
    let mut actual = 0;
    while let Some(chunk) = body.next().await {
        let chunk = chunk?;
        output.write_all(&chunk).await?;
        actual += chunk.len() as u64;
        bar.inc(chunk.len() as u64);
    }
    output.flush().await?;
    if actual != expected {
        return Err(Error::SizeMismatch {
            path: remote.into(),
            expected,
            actual,
        });
    }
    Ok(())
}
fn split_ranges(size: u64, requested: usize) -> Vec<(u64, u64)> {
    let count = (requested as u64).min(size) as usize;
    let base = size / count as u64;
    let remainder = size % count as u64;
    let mut start = 0;
    (0..count)
        .map(|index| {
            let length = base + u64::from((index as u64) < remainder);
            let range = (start, start + length - 1);
            start += length;
            range
        })
        .collect()
}
fn valid_range(value: Option<&str>, offset: u64) -> bool {
    value.is_some_and(|v| v.starts_with(&format!("bytes {offset}-")))
}
fn bar_style(bar: &ProgressBar) {
    bar.set_style(
        ProgressStyle::with_template(
            "{msg:.green} [{bar:30.cyan/blue}] {bytes}/{total_bytes} {bytes_per_sec}",
        )
        .unwrap()
        .progress_chars("=> "),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn range_validation() {
        assert!(valid_range(Some("bytes 5-9/10"), 5));
        assert!(!valid_range(Some("bytes 0-9/10"), 5));
    }
    #[test]
    fn splits_ranges_without_gaps() {
        assert_eq!(split_ranges(10, 3), vec![(0, 3), (4, 6), (7, 9)]);
    }
}
