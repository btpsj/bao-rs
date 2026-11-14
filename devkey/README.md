# devkey

`dev.key` is the Xous project's developer signing key (ed25519, PKCS#8 PEM), copied from
https://github.com/betrusted-io/xous-core/blob/5397e1b488c081566cef2c0e597e05426f67c1c3/devkey/dev.key

sha256: `92c82b6ea40c3259890fa63463197680542e71c0943c16dd0752cbd0387981ce`

It is public by design; the first image signed with it switches the chip to developer mode
(permanently erasing the factory secrets).
