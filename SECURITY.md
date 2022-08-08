# Security Policy

## Supported Versions

Only the latest release is supported.

## Reporting a Vulnerability

If you are aware of a security vulnerability or risk in cargo-mutants, please
contact me directly by mail at <mbp@sourcefrog.net>, rather than filing a public
bug.

I expect to normally respond within one week but this is not guaranteed.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## Security Model

cargo-mutants runs `cargo build` and `cargo test` on the specified source tree,
and on generated mutations of that tree. Rust builds (through `build.rs`) and
tests necessarily provide a means for generic code execution. Malicious code
under test will have control of the test environment. If the code is not trusted
it should be tested within a strong sandbox.
