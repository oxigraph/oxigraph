//! Simple HTTP client

use crate::error::{invalid_data_error, invalid_input_error};
use http::header::{CONNECTION, CONTENT_LENGTH, HOST, TRANSFER_ENCODING};
use http::{Request, Response, Version};
use httparse::Status;
use native_tls::TlsConnector;
use std::cmp::min;
use std::convert::TryInto;
use std::io;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;

pub struct Client {}

impl Client {
    pub fn new() -> Self {
        Self {}
    }

    pub fn request(
        &self,
        request: &Request<Option<Vec<u8>>>,
    ) -> io::Result<Response<Box<dyn BufRead>>> {
        let scheme = request
            .uri()
            .scheme_str()
            .ok_or_else(|| invalid_input_error("No host provided"))?;
        let port = if let Some(port) = request.uri().port_u16() {
            port
        } else {
            match scheme {
                "http" => 80,
                "https" => 443,
                _ => {
                    return Err(invalid_input_error(format!(
                        "No port provided for scheme '{}'",
                        scheme
                    )))
                }
            }
        };
        let host = request
            .uri()
            .host()
            .ok_or_else(|| invalid_input_error("No host provided"))?;

        match scheme {
            "http" => {
                let mut stream = TcpStream::connect((host, port))?;
                self.encode(request, &mut stream)?;
                self.decode(stream)
            }
            "https" => {
                let connector =
                    TlsConnector::new().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                let stream = TcpStream::connect((host, port))?;
                let mut stream = connector
                    .connect(host, stream)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                self.encode(request, &mut stream)?;
                self.decode(stream)
            }
            _ => Err(invalid_input_error(format!(
                "Not supported URL scheme: {}",
                scheme
            ))),
        }
    }

    fn encode(
        &self,
        request: &Request<Option<Vec<u8>>>,
        mut writer: &mut impl Write,
    ) -> io::Result<()> {
        if request.headers().contains_key(CONTENT_LENGTH) {
            return Err(invalid_input_error(
                "content-length header is set by the client library",
            ));
        }
        if request.headers().contains_key(HOST) {
            return Err(invalid_input_error(
                "host header is set by the client library",
            ));
        }
        if request.headers().contains_key(CONNECTION) {
            return Err(invalid_input_error(
                "connection header is set by the client library",
            ));
        }
        if let Some(query) = request.uri().query() {
            write!(
                &mut writer,
                "{} {}?{} HTTP/1.1\r\n",
                request.method(),
                request.uri().path(),
                query
            )?;
        } else {
            write!(
                &mut writer,
                "{} {} HTTP/1.1\r\n",
                request.method(),
                request.uri().path()
            )?;
        }

        // host
        let host = request
            .uri()
            .host()
            .ok_or_else(|| invalid_input_error("No host provided"))?;
        if let Some(port) = request.uri().port() {
            write!(writer, "host: {}:{}\r\n", request.uri(), port)
        } else {
            write!(writer, "host: {}\r\n", host)
        }?;

        // connection
        write!(writer, "connection: close\r\n")?;

        // headers
        for (name, value) in request.headers() {
            write!(writer, "{}: ", name.as_str())?;
            writer.write_all(value.as_bytes())?;
            write!(writer, "\r\n")?;
        }

        // body with content-length
        if let Some(payload) = request.body() {
            write!(writer, "content-length: {}\r\n\r\n", payload.len())?;
            writer.write_all(payload)?;
        } else {
            write!(writer, "\r\n")?;
        }
        Ok(())
    }

    fn decode<'a>(&self, reader: impl Read + 'a) -> io::Result<Response<Box<dyn BufRead + 'a>>> {
        let mut reader = BufReader::new(reader);

        // Let's read the headers
        let mut buffer = Vec::new();
        let mut headers = [httparse::EMPTY_HEADER; 1024];
        let mut parsed_response = httparse::Response::new(&mut headers);
        loop {
            if reader.read_until(b'\n', &mut buffer)? == 0 {
                return Err(invalid_data_error("Empty HTTP response"));
            }
            if buffer.len() > 8 * 1024 {
                return Err(invalid_data_error("The headers size should fit in 8kb"));
            }

            if buffer.ends_with(b"\r\n\r\n") || buffer.ends_with(b"\n\n") {
                break; //end of buffer
            }
        }
        if parsed_response
            .parse(&buffer)
            .map_err(invalid_data_error)?
            .is_partial()
        {
            return Err(invalid_input_error(
                "Partial HTTP headers containing two line jumps",
            ));
        }

        // Let's build the response
        let mut response = Response::builder()
            .status(
                parsed_response
                    .code
                    .ok_or_else(|| invalid_data_error("No status code in the HTTP response"))?,
            )
            .version(match parsed_response.version {
                Some(0) => Version::HTTP_10,
                Some(1) => Version::HTTP_11,
                Some(id) => {
                    return Err(invalid_data_error(format!(
                        "Unsupported HTTP response version: 1.{}",
                        id
                    )))
                }
                None => return Err(invalid_data_error("No HTTP version in the HTTP response")),
            });
        for header in parsed_response.headers {
            response = response.header(header.name, header.value);
        }

        let content_length = response.headers_ref().and_then(|h| h.get(CONTENT_LENGTH));
        let transfer_encoding = response
            .headers_ref()
            .and_then(|h| h.get(TRANSFER_ENCODING));
        if transfer_encoding.is_some() && content_length.is_some() {
            return Err(invalid_data_error(
                "Transfer-Encoding and Content-Length should not be set at the same time",
            ));
        }

        let body: Box<dyn BufRead> = if let Some(content_length) = content_length {
            let len = content_length
                .to_str()
                .map_err(invalid_data_error)?
                .parse::<u64>()
                .map_err(invalid_data_error)?;
            Box::new(reader.take(len))
        } else if let Some(transfer_encoding) = transfer_encoding {
            let transfer_encoding = transfer_encoding.to_str().map_err(invalid_data_error)?;
            if transfer_encoding.eq_ignore_ascii_case("chunked") {
                buffer.clear();
                Box::new(BufReader::new(ChunkedResponse {
                    reader,
                    buffer,
                    is_start: true,
                    chunk_position: 1,
                    chunk_size: 1,
                }))
            } else {
                return Err(invalid_data_error(format!(
                    "Transfer-Encoding: {} is not supported",
                    transfer_encoding
                )));
            }
        } else {
            Box::new(io::empty())
        };

        response.body(body).map_err(invalid_data_error)
    }
}

struct ChunkedResponse<R: BufRead> {
    reader: R,
    buffer: Vec<u8>,
    is_start: bool,
    chunk_position: usize,
    chunk_size: usize,
}

impl<R: BufRead> Read for ChunkedResponse<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            // In case we still have data
            if self.chunk_position < self.chunk_size {
                let inner_buf = self.reader.fill_buf()?;
                let size = min(
                    min(buf.len(), inner_buf.len()),
                    self.chunk_size - self.chunk_position,
                );
                buf[..size].copy_from_slice(&inner_buf[..size]);
                self.reader.consume(size);
                self.chunk_position += size;
                return Ok(size); // Won't be 0 if there is still some inner buffer
            }

            if self.chunk_size == 0 {
                return Ok(0); // We know it's the end
            }

            if self.is_start {
                self.is_start = false;
            } else {
                // chunk end
                self.buffer.clear();
                self.reader.read_until(b'\n', &mut self.buffer)?;
                if self.buffer != b"\r\n" && self.buffer != b"\n" {
                    return Err(invalid_data_error("Invalid chunked element end"));
                }
            }

            // We load a new chunk
            self.buffer.clear();
            self.reader.read_until(b'\n', &mut self.buffer)?;
            self.chunk_position = 0;
            self.chunk_size = if let Ok(Status::Complete((read, chunk_size))) =
                httparse::parse_chunk_size(&self.buffer)
            {
                if read != self.buffer.len() {
                    return Err(invalid_data_error("Chuncked header containing a line jump"));
                }
                chunk_size.try_into().map_err(invalid_data_error)?
            } else {
                return Err(invalid_data_error("Invalid chuncked header"));
            };

            if self.chunk_size == 0 {
                // we read the trailers
                loop {
                    if self.reader.read_until(b'\n', &mut self.buffer)? == 0 {
                        return Err(invalid_data_error("Missing chunked encoding end"));
                    }
                    if self.buffer.len() > 8 * 1024 {
                        return Err(invalid_data_error("The trailers size should fit in 8kb"));
                    }
                    if self.buffer.ends_with(b"\r\n\r\n") || self.buffer.ends_with(b"\n\n") {
                        break; //end of buffer
                    }
                }
                return Ok(0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::header::{ACCEPT, CONTENT_TYPE};
    use http::{Method, StatusCode};
    use std::io::Cursor;
    use std::str;

    #[test]
    fn encode_get_request() -> io::Result<()> {
        let mut buffer = Vec::new();
        Client::new().encode(
            &Request::builder()
                .method(Method::GET)
                .uri("http://example.com/foo/bar?query#fragment")
                .header(ACCEPT, "application/json")
                .body(None)
                .unwrap(),
            &mut buffer,
        )?;
        assert_eq!(
            str::from_utf8(&buffer).unwrap(),
            "GET /foo/bar?query HTTP/1.1\r\nhost: example.com\r\nconnection: close\r\naccept: application/json\r\n\r\n"
        );
        Ok(())
    }

    #[test]
    fn encode_post_request() -> io::Result<()> {
        let mut buffer = Vec::new();
        Client::new().encode(
            &Request::builder()
                .method(Method::POST)
                .uri("http://example.com/foo/bar?query#fragment")
                .header(ACCEPT, "application/json")
                .body(Some(b"testbody".to_vec()))
                .unwrap(),
            &mut buffer,
        )?;
        assert_eq!(
            str::from_utf8(&buffer).unwrap(),
            "POST /foo/bar?query HTTP/1.1\r\nhost: example.com\r\nconnection: close\r\naccept: application/json\r\ncontent-length: 8\r\n\r\ntestbody"
        );
        Ok(())
    }

    #[test]
    fn decode_response_without_payload() -> io::Result<()> {
        let response = Client::new()
            .decode(Cursor::new("HTTP/1.1 404 Not Found\r\n\r\n"))
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let mut buf = String::new();
        response.into_body().read_to_string(&mut buf)?;
        assert!(buf.is_empty());
        Ok(())
    }

    #[test]
    fn decode_response_with_fixed_payload() -> io::Result<()> {
        let response = Client::new().decode(Cursor::new(
            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length:8\r\n\r\ntestbody",
        ))?;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            "text/plain"
        );
        let mut buf = String::new();
        response.into_body().read_to_string(&mut buf)?;
        assert_eq!(buf, "testbody");
        Ok(())
    }

    #[test]
    fn decode_response_with_chunked_payload() -> io::Result<()> {
        let response = Client::new().decode(Cursor::new(
            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ntransfer-encoding:chunked\r\n\r\n4\r\nWiki\r\n5\r\npedia\r\nE\r\n in\r\n\r\nchunks.\r\n0\r\n\r\n",
        ))?;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            "text/plain"
        );
        let mut buf = String::new();
        response.into_body().read_to_string(&mut buf)?;
        assert_eq!(buf, "Wikipedia in\r\n\r\nchunks.");
        Ok(())
    }

    #[test]
    fn decode_response_with_trailer() -> io::Result<()> {
        let response = Client::new().decode(Cursor::new(
            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ntransfer-encoding:chunked\r\n\r\n4\r\nWiki\r\n5\r\npedia\r\nE\r\n in\r\n\r\nchunks.\r\n0\r\ntest: foo\r\n\r\n",
        ))?;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            "text/plain"
        );
        let mut buf = String::new();
        response.into_body().read_to_string(&mut buf)?;
        assert_eq!(buf, "Wikipedia in\r\n\r\nchunks.");
        Ok(())
    }
}
