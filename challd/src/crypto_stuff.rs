use x25519_dalek::{StaticSecret, PublicKey};

pub struct Keypair {
	pub pubkey: PublicKey,
	pub privkey: StaticSecret
}

pub fn gen_keypair() -> Keypair {
	let privkey = StaticSecret::random();
	let pubkey = (&privkey).into();

	Keypair {
		pubkey,
		privkey
	}
}
