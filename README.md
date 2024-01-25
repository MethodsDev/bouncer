# bouncer

This is a Python+Rust package for barcode correction. It is built on top of [SymSpell](https://github.com/wolfgarbe/SymSpell) (specifically [this Rust implementation](https://github.com/reneklacan/symspell)), but we have specialized the algorithm for the barcode problem as [`barcode-symspell`](https://github.com/MethodsDev/barcode-symspell). Barcode correction has some notable differences compared to spell-checking:

* The word list can be much larger: there are many millions of possible barcodes.
* Barcode length is much higher than the average word length in most languages. 16bp barcodes are common, 39bp are not unheard of.
* Frequency is not necessarily relevant&mdash;when initially calling barcodes, they are all equally likely. A second round based on observed counts might be useful as a future direction.
* SymSpell uses a prefix to save space, but this doesn't work as well with random barcodes. `barcode-symspell` uses a pigeonhole strategy to dramatically cut down the number of deletions stored.
* The alphabet is much smaller. SymSpell is not affected by alphabet size but it's possible this could allow for some space savings for the index. That said, bytes are small enough and easy to work with.

### Installation

Currently installation has to be done from source. This means you get to install Rust, see [rustup.rs](https://rustup.rs/) for platform-specific instructions.

Once you have Rust on your system and available, `pip` should be able to install the package. You can install directly from GitHub with `pip install git+ssh://git@github.com/MethodsDev/bouncer.git`, or you can clone the repo (along with the submodule `barcode-symspell`) and install manually: 

```sh
git clone --recurse-submodules git@github.com:MethodsDev/bouncer.git
cd bouncer
pip install .
```
