import exception
import filepath
import gleam/bit_array
import gleam/bool
import gleam/bytes_tree.{type BytesTree}
import gleam/crypto
import gleam/dict.{type Dict}
import gleam/dynamic.{type Dynamic}
import gleam/dynamic/decode
import gleam/erlang/application
import gleam/erlang/atom.{type Atom}
import gleam/http.{type Method}
import gleam/http/cookie
import gleam/http/request.{type Request as HttpRequest}
import gleam/http/response.{
  type Response as HttpResponse, Response as HttpResponse,
}
import gleam/int
import gleam/json
import gleam/list
import gleam/option.{type Option}
import gleam/result
import gleam/string
import gleam/string_tree.{type StringTree}
import gleam/uri
import houdini
import logging
import marceau
import simplifile
import wisp/internal

//
// Responses
//

/// The body of a HTTP response, to be sent to the client.
///
pub type Body {
  /// A body of unicode text.
  ///
  /// If you have a `StringTree` you can use the `bytes_tree.from_string_tree`
  /// function with the `Bytes` variant instead, as this will avoid the cost of
  /// converting the tree into a string.
  ///
  Text(String)
  /// A body of binary data, stored as a `BytesTree`.
  ///
  /// If you have a `BitArray` you can use the `bytes_tree.from_bit_array`
  /// function to convert it.
  ///
  /// If you have a `StringTree` you can use the `bytes_tree.from_string_tree`
  /// function to convert it.
  ///
  Bytes(BytesTree)
  /// A body of the contents of a file.
  ///
  /// This will be sent efficiently using the `send_file` function of the
  /// underlying HTTP server. The file will not be read into memory so it is
  /// safe to send large files this way.
  ///
  File(
    /// The path to the file on the server file system.
    path: String,
    /// The number of bytes to skip from the start of the file. Set to 0 for the whole file.
    offset: Int,
    /// The maximum number of bytes to send. Set to `None` for the whole file.
    limit: Option(Int),
  )
}

/// An alias for a HTTP response containing a `Body`.
pub type Response =
  HttpResponse(Body)

/// Create a response with the given status code.
///
/// # Examples
///
/// ```gleam
/// response(200)
/// // -> Response(200, [], Text(""))
/// ```
///
pub fn response(status: Int) -> Response {
  HttpResponse(status, [], Text(""))
}

/// Set the body of a response.
///
/// # Examples
///
/// ```gleam
/// response(200)
/// |> set_body(File("/tmp/myfile.txt", option.None))
/// // -> Response(200, [], File("/tmp/myfile.txt", option.None))
/// ```
///
pub fn set_body(response: Response, body: Body) -> Response {
  response
  |> response.set_body(body)
}

/// Send a file from the disc as a file download.
///
/// The operating system `send_file` function is used to efficiently send the
/// file over the network socket without reading the entire file into memory.
///
/// The `content-disposition` header will be set to `attachment;
/// filename="name"` to ensure the file is downloaded by the browser. This is
/// especially good for files that the browser would otherwise attempt to open
/// as this can result in cross-site scripting vulnerabilities.
///
/// If you wish to not set the `content-disposition` header you could use the
/// `set_body` function with the `File` body variant.
///
/// # Examples
///
/// ```gleam
/// response(200)
/// |> file_download(named: "myfile.txt", from: "/tmp/myfile.txt")
