# bouncer

This is a Python+Rust package for barcode correction. It is built on top of [SymSpell](https://github.com/wolfgarbe/SymSpell) (specifically [this Rust implementation](https://github.com/reneklacan/symspell)), but we have specialized the algorithm for the barcode problem. Barcode correction has some notable differences compared to spell-checking:

* The word list can be much larger: there are many millions of possible barcodes.
* Barcode length is much higher than the average word length in most languages. 16bp barcodes are common, 39bp are not unheard of.
* Frequency is not necessarily relevant&mdash;when initially calling barcodes, they are all equally likely. A second round based on observed counts might be useful as a future direction.
* SymSpell uses a prefix to cut down on storage, but a prefix is of limited use with a random barcode. Better strategies are possible but need to be implemented.
* The alphabet is much smaller. SymSpell is not affected by alphabet size but it's possible this could allow for some space savings for the index.
