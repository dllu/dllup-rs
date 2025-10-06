use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

pub trait MathEngine {
    fn tex_to_html(&mut self, latex: &str, inline: bool) -> Result<String, String>;
}

pub struct ExternalCmdEngine {
    pub cmd: Vec<String>,
}

impl MathEngine for ExternalCmdEngine {
    fn tex_to_html(&mut self, latex: &str, inline: bool) -> Result<String, String> {
        if self.cmd.is_empty() {
            return Err("no command configured".into());
        }
        let mut parts = self.cmd.clone();
        // KaTeX CLI: read TeX from stdin, write HTML to stdout; add display flag if needed
        if !inline {
            parts.push("--display-mode".to_string());
            dbg!(&latex);
        }
        let (prog, args) = parts.split_first().unwrap();
        let mut child = Command::new(prog)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to spawn math command: {}", e))?;
        {
            let stdin = child.stdin.as_mut().ok_or("failed to open stdin")?;
            stdin
                .write_all(latex.as_bytes())
                .map_err(|e| format!("failed to write TeX to stdin: {}", e))?;
        }
        let out = child
            .wait_with_output()
            .map_err(|e| format!("failed waiting for math command: {}", e))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).to_string())
        } else {
            Err(format!(
                "math command failed: status {} stderr {}",
                out.status,
                String::from_utf8_lossy(&out.stderr)
            ))
        }
    }
}

pub struct PersistentKatexEngine {
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

#[derive(Serialize, Deserialize)]
struct KatexReq<'a> {
    tex: &'a str,
    inline: bool,
}
#[derive(Serialize, Deserialize)]
struct KatexResp {
    html: String,
}

impl PersistentKatexEngine {
    pub fn spawn() -> Result<Self, String> {
        // Node inline script: loads katex once, reads JSON lines on stdin, writes JSON lines on stdout
        let script = r#"const katex = require('katex');
const rl = require('readline').createInterface({ input: process.stdin, crlfDelay: Infinity });
rl.on('line', (line) => {
  try {
    const m = JSON.parse(line);
    const html = katex.renderToString(m.tex, { displayMode: !m.inline, throwOnError: false });
    process.stdout.write(JSON.stringify({ html }) + '\n');
  } catch (e) {
    process.stdout.write(JSON.stringify({ html: '' }) + '\n');
  }
});"#;
        let mut child = Command::new("node")
            .arg("-e")
            .arg(script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to spawn node: {}", e))?;
        let stdin = child.stdin.take().ok_or("failed to open node stdin")?;
        let stdout = child.stdout.take().ok_or("failed to open node stdout")?;
        Ok(Self {
            _child: child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }
}

impl MathEngine for PersistentKatexEngine {
    fn tex_to_html(&mut self, latex: &str, inline: bool) -> Result<String, String> {
        let req = KatexReq { tex: latex, inline };
        let line = serde_json::to_string(&req).map_err(|e| e.to_string())? + "\n";
        self.stdin
            .write_all(line.as_bytes())
            .map_err(|e| e.to_string())?;
        let mut out = String::new();
        self.stdout.read_line(&mut out).map_err(|e| e.to_string())?;
        if out.is_empty() {
            return Err("no response from katex child".into());
        }
        let resp: KatexResp = serde_json::from_str(out.trim_end()).map_err(|e| e.to_string())?;
        Ok(resp.html)
    }
}
