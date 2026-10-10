use git_agent_plugin_sdk::DiffFile;
use serde_json::Value;

// Git quotes UTF-8 paths as octal bytes, unlike JSON string escaping.
fn unquote(value: &str) -> String {
    let Some(value) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) else {
        return value.to_owned();
    };
    let mut bytes = Vec::new();
    let mut source = value.bytes().peekable();
    while let Some(byte) = source.next() {
        if byte != b'\\' {
            bytes.push(byte);
            continue;
        }
        let Some(escaped) = source.next() else { break };
        if (b'0'..=b'7').contains(&escaped) {
            let mut number = u16::from(escaped - b'0');
            for _ in 0..2 {
                if source.peek().is_some_and(|c| (b'0'..=b'7').contains(c)) {
                    number = number * 8 + u16::from(source.next().unwrap() - b'0');
                }
            }
            bytes.push(number as u8);
        } else {
            bytes.push(match escaped {
                b't' => b'\t',
                b'n' => b'\n',
                b'r' => b'\r',
                c => c,
            });
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
fn path(patch: &str) -> Option<String> {
    for prefix in ["+++ ", "rename to ", "--- "] {
        if let Some(line) = patch.lines().find_map(|line| line.strip_prefix(prefix)) {
            let value = unquote(line);
            if value != "/dev/null" {
                return Some(if prefix == "rename to " {
                    value
                } else {
                    value
                        .strip_prefix("b/")
                        .or_else(|| value.strip_prefix("a/"))
                        .unwrap_or(&value)
                        .to_owned()
                });
            }
        }
    }
    // Binary diffs have no ---/+++; the destination token is the last header path.
    let header = patch.lines().next()?.strip_prefix("diff --git ")?;
    if let Some((_, destination)) = header.rsplit_once(" b/") {
        return Some(destination.to_owned());
    }
    let (_, destination) = header.split_once("\" \"")?;
    Some(
        unquote(&format!("\"{destination}"))
            .strip_prefix("b/")?
            .to_owned(),
    )
}
pub(super) fn collect(pr: &Value, diff: &str) -> Vec<DiffFile> {
    let mut files: Vec<DiffFile> = pr["files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|file| {
            Some(DiffFile {
                path: file["path"].as_str()?.into(),
                patch: String::new(),
                additions: file["additions"].as_u64().unwrap_or(0),
                deletions: file["deletions"].as_u64().unwrap_or(0),
                truncated: false,
            })
        })
        .collect();
    let starts: Vec<usize> = diff
        .match_indices("diff --git ")
        .filter_map(|(offset, _)| {
            (offset == 0 || diff.as_bytes()[offset - 1] == b'\n').then_some(offset)
        })
        .collect();
    for (index, start) in starts.iter().enumerate() {
        let patch = &diff[*start..starts.get(index + 1).copied().unwrap_or(diff.len())];
        let Some(path) = path(patch) else { continue };
        if let Some(file) = files.iter_mut().find(|file| file.path == path) {
            file.patch = patch.into();
        } else {
            files.push(DiffFile {
                path,
                patch: patch.into(),
                additions: patch
                    .lines()
                    .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
                    .count() as u64,
                deletions: patch
                    .lines()
                    .filter(|l| l.starts_with('-') && !l.starts_with("---"))
                    .count() as u64,
                truncated: false,
            });
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn collects_added_deleted_renamed_binary_and_quoted_files() {
        let diff = "diff --git a/a b/a\n--- /dev/null\n+++ b/a\n@@ -0,0 +1 @@\n+new\ndiff --git a/gone b/gone\n--- a/gone\n+++ /dev/null\n@@ -1 +0,0 @@\n-old\ndiff --git a/old b/new name\nsimilarity index 100%\nrename from old\nrename to new name\ndiff --git a/image.png b/image.png\nBinary files a/image.png and b/image.png differ\ndiff --git \"a/\\344\\270\\255.txt\" \"b/\\344\\270\\255.txt\"\n--- \"a/\\344\\270\\255.txt\"\n+++ \"b/\\344\\270\\255.txt\"\n@@ -1 +1 @@\n-before\n+after\n";
        let files = collect(
            &json!({"files":[{"path":"a","additions":1,"deletions":0},{"path":"not-in-patch","additions":0,"deletions":0}]}),
            diff,
        );
        assert_eq!(files.len(), 6);
        assert_eq!(
            files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(),
            vec![
                "a",
                "not-in-patch",
                "gone",
                "new name",
                "image.png",
                "中.txt"
            ]
        );
        assert_eq!(files[0].additions, 1);
        assert_eq!(files[2].deletions, 1);
        assert!(files[1].patch.is_empty());
        assert!(files[4].patch.contains("Binary files"));
        assert!(!files[0].patch.contains("gone"));
    }
}
