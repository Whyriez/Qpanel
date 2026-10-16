use base64::prelude::*;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::debug;

const SMTP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub encryption: String,
    pub from_address: String,
    pub from_name: String,
}

#[derive(Debug, Clone)]
pub struct EmailMessage {
    pub to: String,
    pub subject: String,
    pub body: String,
}

pub struct SmtpClient;

impl SmtpClient {
    /// Tests connectivity and optional authentication with the target SMTP server.
    pub async fn test_connection(config: &SmtpConfig) -> Result<(), String> {
        let mut session = SmtpSession::connect(config).await?;
        session.handshake(config).await?;
        session.quit().await?;
        Ok(())
    }

    /// Sends an email message asynchronously over SMTP.
    pub async fn send_mail(config: &SmtpConfig, msg: &EmailMessage) -> Result<(), String> {
        let mut session = SmtpSession::connect(config).await?;
        session.handshake(config).await?;
        session.send_message(config, msg).await?;
        session.quit().await?;
        Ok(())
    }
}

struct SmtpSession {
    reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: tokio::net::tcp::OwnedWriteHalf,
}

impl SmtpSession {
    async fn connect(config: &SmtpConfig) -> Result<Self, String> {
        let addr = format!("{}:{}", config.host, config.port);
        let stream = timeout(SMTP_TIMEOUT, TcpStream::connect(&addr))
            .await
            .map_err(|_| format!("Koneksi SMTP ke {addr} timeout setelah 10 detik"))?
            .map_err(|e| format!("Gagal terhubung ke SMTP {addr}: {e}"))?;

        let (read_half, writer) = stream.into_split();
        let mut reader = BufReader::new(read_half);

        let mut line = String::new();
        let read_res = timeout(SMTP_TIMEOUT, reader.read_line(&mut line))
            .await
            .map_err(|_| "Timeout membaca banner awal SMTP".to_string())?
            .map_err(|e| format!("Error membaca banner SMTP: {e}"))?;

        if read_res == 0 || !line.starts_with("220") {
            return Err(format!("Banner SMTP tidak valid: {}", line.trim()));
        }

        debug!("SMTP Connected: {}", line.trim());
        Ok(Self { reader, writer })
    }

    async fn send_command(&mut self, cmd: &str) -> Result<(), String> {
        self.writer
            .write_all(format!("{cmd}\r\n").as_bytes())
            .await
            .map_err(|e| format!("Gagal mengirim perintah SMTP: {e}"))?;
        self.writer
            .flush()
            .await
            .map_err(|e| format!("Gagal flush perintah SMTP: {e}"))
    }

    async fn read_multiline_reply(&mut self) -> Result<(u16, Vec<String>), String> {
        let mut lines = Vec::new();
        loop {
            let mut line = String::new();
            let n = timeout(SMTP_TIMEOUT, self.reader.read_line(&mut line))
                .await
                .map_err(|_| "Timeout membaca respons SMTP".to_string())?
                .map_err(|e| format!("Gagal membaca respons SMTP: {e}"))?;

            if n == 0 {
                return Err("Koneksi SMTP ditutup oleh server".to_string());
            }

            let trimmed = line.trim_end();
            if trimmed.len() < 3 {
                return Err(format!("Respons SMTP tidak valid: '{trimmed}'"));
            }

            lines.push(trimmed.to_string());

            let is_hyphen = trimmed.as_bytes().get(3) == Some(&b'-');
            if !is_hyphen {
                let code_str = &trimmed[..3];
                let code: u16 = code_str
                    .parse()
                    .map_err(|_| format!("Kode status SMTP tidak valid: '{code_str}'"))?;
                return Ok((code, lines));
            }
        }
    }

    async fn handshake(&mut self, config: &SmtpConfig) -> Result<(), String> {
        self.send_command("EHLO localhost").await?;
        let (code, _lines) = self.read_multiline_reply().await?;
        if code != 250 {
            // Fallback to HELO if EHLO not accepted
            self.send_command("HELO localhost").await?;
            let (helo_code, _) = self.read_multiline_reply().await?;
            if helo_code != 250 {
                return Err(format!("Server menolak HELO/EHLO (status {helo_code})"));
            }
        }

        // Authentication if credentials supplied
        if let (Some(user), Some(pass)) = (&config.username, &config.password)
            && !user.is_empty()
        {
            self.send_command("AUTH LOGIN").await?;
            let (auth_code, _) = self.read_multiline_reply().await?;
            if auth_code != 334 {
                return Err(format!("AUTH LOGIN ditolak (status {auth_code})"));
            }

            // Send base64 username
            let user_b64 = BASE64_STANDARD.encode(user.as_bytes());
            self.send_command(&user_b64).await?;
            let (user_code, _) = self.read_multiline_reply().await?;
            if user_code != 334 {
                return Err(format!("Username SMTP ditolak (status {user_code})"));
            }

            // Send base64 password
            let pass_b64 = BASE64_STANDARD.encode(pass.as_bytes());
            self.send_command(&pass_b64).await?;
            let (pass_code, _) = self.read_multiline_reply().await?;
            if pass_code != 235 {
                return Err(format!("Autentikasi SMTP gagal (status {pass_code})"));
            }
            debug!("SMTP Authenticated successfully as {}", user);
        }

        Ok(())
    }

    async fn send_message(
        &mut self,
        config: &SmtpConfig,
        msg: &EmailMessage,
    ) -> Result<(), String> {
        self.send_command(&format!("MAIL FROM:<{}>", config.from_address))
            .await?;
        let (from_code, _) = self.read_multiline_reply().await?;
        if from_code != 250 {
            return Err(format!(
                "Alamat pengirim ditolak oleh server (status {from_code})"
            ));
        }

        self.send_command(&format!("RCPT TO:<{}>", msg.to)).await?;
        let (to_code, _) = self.read_multiline_reply().await?;
        if to_code != 250 {
            return Err(format!(
                "Alamat penerima '{}' ditolak oleh server (status {to_code})",
                msg.to
            ));
        }

        self.send_command("DATA").await?;
        let (data_code, _) = self.read_multiline_reply().await?;
        if data_code != 354 {
            return Err(format!("Server menolak payload DATA (status {data_code})"));
        }

        // Format headers & body
        let header_from = if config.from_name.is_empty() {
            format!("From: <{}>\r\n", config.from_address)
        } else {
            format!(
                "From: \"{}\" <{}>\r\n",
                config.from_name, config.from_address
            )
        };
        let header_to = format!("To: <{}>\r\n", msg.to);
        let header_subj = format!("Subject: {}\r\n", msg.subject);
        let mime_header = "MIME-Version: 1.0\r\nContent-Type: text/plain; charset=UTF-8\r\n";

        // Dot stuffing per RFC 5321 (lines starting with '.' must be prefixed with another '.')
        let mut sanitized_body = String::new();
        for line in msg.body.lines() {
            if line.starts_with('.') {
                sanitized_body.push('.');
            }
            sanitized_body.push_str(line);
            sanitized_body.push_str("\r\n");
        }

        let full_payload = format!(
            "{}{}{}{}\r\n{}.\r\n",
            header_from, header_to, header_subj, mime_header, sanitized_body
        );

        self.writer
            .write_all(full_payload.as_bytes())
            .await
            .map_err(|e| format!("Gagal menulis payload email: {e}"))?;
        self.writer
            .flush()
            .await
            .map_err(|e| format!("Gagal flush payload email: {e}"))?;

        let (sent_code, _) = self.read_multiline_reply().await?;
        if sent_code != 250 {
            return Err(format!("Pengiriman email ditolak (status {sent_code})"));
        }

        debug!("Email successfully sent to {}", msg.to);
        Ok(())
    }

    async fn quit(&mut self) -> Result<(), String> {
        let _ = self.send_command("QUIT").await;
        let _ = self.read_multiline_reply().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_smtp_mock_server_flow() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let local_addr = listener.local_addr().expect("local addr");

        // Spawn mock SMTP server in background
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let (r, mut w) = stream.split();
                let mut reader = BufReader::new(r);
                let _ = w
                    .write_all(b"220 mock.smtp.local Qpanel SMTP Test\r\n")
                    .await;

                let mut line = String::new();
                while let Ok(n) = reader.read_line(&mut line).await {
                    if n == 0 {
                        break;
                    }
                    let trimmed = line.trim();
                    if trimmed.starts_with("EHLO") {
                        let _ = w
                            .write_all(b"250-mock.smtp.local\r\n250-AUTH LOGIN\r\n250 8BITMIME\r\n")
                            .await;
                    } else if trimmed == "AUTH LOGIN" {
                        let _ = w.write_all(b"334 VXNlcm5hbWU6\r\n").await;
                    } else if trimmed == "dXNlcm5hbWU=" {
                        // "username"
                        let _ = w.write_all(b"334 UGFzc3dvcmQ6\r\n").await;
                    } else if trimmed == "cGFzc3dvcmQ=" {
                        // "password"
                        let _ = w
                            .write_all(b"235 2.7.0 Authentication successful\r\n")
                            .await;
                    } else if trimmed.starts_with("MAIL FROM:") {
                        let _ = w.write_all(b"250 2.1.0 Sender OK\r\n").await;
                    } else if trimmed.starts_with("RCPT TO:") {
                        let _ = w.write_all(b"250 2.1.5 Recipient OK\r\n").await;
                    } else if trimmed == "DATA" {
                        let _ = w
                            .write_all(b"354 Start mail input; end with <CRLF>.<CRLF>\r\n")
                            .await;
                    } else if trimmed == "." {
                        let _ = w.write_all(b"250 2.0.0 OK queued\r\n").await;
                    } else if trimmed == "QUIT" {
                        let _ = w.write_all(b"221 2.0.0 Bye\r\n").await;
                        break;
                    }
                    line.clear();
                }
            }
        });

        let config = SmtpConfig {
            host: "127.0.0.1".to_string(),
            port: local_addr.port(),
            username: Some("username".to_string()),
            password: Some("password".to_string()),
            encryption: "none".to_string(),
            from_address: "panel@local".to_string(),
            from_name: "Qpanel".to_string(),
        };

        // Test connection
        let conn_res = SmtpClient::test_connection(&config).await;
        assert!(
            conn_res.is_ok(),
            "Test connection failed: {:?}",
            conn_res.err()
        );

        // Test sending mail
        let send_res = SmtpClient::send_mail(
            &config,
            &EmailMessage {
                to: "admin@local".to_string(),
                subject: "[Test] Alert".to_string(),
                body: "This is a test notification message.".to_string(),
            },
        )
        .await;
        assert!(send_res.is_ok(), "Send mail failed: {:?}", send_res.err());
    }
}
