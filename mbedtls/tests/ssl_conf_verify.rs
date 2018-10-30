/* Copyright (c) Fortanix, Inc.
 *
 * Licensed under the GNU General Public License, version 2 <LICENSE-GPL or 
 * https://www.gnu.org/licenses/gpl-2.0.html> or the Apache License, Version 
 * 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>, at your 
 * option. This file may not be copied, modified, or distributed except 
 * according to those terms. */

extern crate mbedtls;

use std::net::TcpStream;

use mbedtls::Error;
use mbedtls::Result as TlsResult;
use mbedtls::rng::CtrDrbg;
use mbedtls::x509::{Certificate, LinkedCertificate, VerifyError, verify_error};
use mbedtls::pk::Pk;
use mbedtls::ssl::config::{Endpoint, Transport, Preset};
use mbedtls::ssl::{Config, Context};

mod support;
use support::keys;
use support::entropy::entropy_new;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Test {
	CallbackSetVerifyFlags,
	CallbackError,
}

fn client(mut conn: TcpStream, test: Test) -> TlsResult<()> {
	let mut entropy = entropy_new();
	let mut rng = try!(CtrDrbg::new(&mut entropy, None));
	let mut cert = try!(Certificate::from_pem(keys::PEM_CERT));
	let verify_callback = &mut |_: &mut LinkedCertificate, _, verify_flags: &mut VerifyError| {
		match test {
			Test::CallbackSetVerifyFlags => {
				*verify_flags |= verify_error::CERT_OTHER;
				Ok(())
			}
			Test::CallbackError => {
				Err(Error::Asn1InvalidData)
			}
		}
	};
	let mut config = Config::new(Endpoint::Client, Transport::Stream, Preset::Default);
	config.set_rng(Some(&mut rng));
	config.set_verify_callback(verify_callback);
	config.set_ca_list(Some(&mut *cert), None);
	let mut ctx = try!(Context::new(&config));

	match (test, ctx.establish(&mut conn, None).err().expect("should have failed")) {
		(Test::CallbackSetVerifyFlags, Error::X509CertVerifyFailed) => {}
		(Test::CallbackError, Error::Asn1InvalidData) => {}
		(_, err) => assert!(false, "Unexpected error from ctx.establish(): {:?}", err),
	}

	Ok(())
}

fn server(mut conn: TcpStream) -> TlsResult<()> {
	let mut entropy = entropy_new();
	let mut rng = try!(CtrDrbg::new(&mut entropy, None));
	let mut cert = try!(Certificate::from_pem(keys::PEM_CERT));
	let mut key = try!(Pk::from_private_key(keys::PEM_KEY, None));
	let mut config = Config::new(Endpoint::Server, Transport::Stream, Preset::Default);
	config.set_rng(Some(&mut rng));
	try!(config.push_cert(&mut *cert, &mut key));
	let mut ctx = try!(Context::new(&config));

	let _ = ctx.establish(&mut conn, None);
	Ok(())
}

#[cfg(unix)]
mod test {
	use std::thread;
	use support::net::create_tcp_pair;

	#[test]
	fn callback_set_verify_flags() {
		let (c, s) = create_tcp_pair().unwrap();

		let c = thread::spawn(move || super::client(c, super::Test::CallbackSetVerifyFlags).unwrap());
		let s = thread::spawn(move || super::server(s).unwrap());
		c.join().unwrap();
		s.join().unwrap();
	}

	#[test]
	fn callback_error() {
		let (c, s) = create_tcp_pair().unwrap();

		let c = thread::spawn(move || super::client(c, super::Test::CallbackError).unwrap());
		let s = thread::spawn(move || super::server(s).unwrap());
		c.join().unwrap();
		s.join().unwrap();
	}
}
