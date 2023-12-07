use std::fs::File;
use std::io::{self, prelude::*};
use std::path::PathBuf;

use flate2::read::GzDecoder;
use hashbrown::HashSet;
use log::{debug, info, trace};

use pyo3::exceptions::{PyIOError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;

use barcode_symspell::{SymSpell, SymSpellBuilder};

fn read_barcodes(barcode_file: PathBuf) -> io::Result<Vec<String>> {
    let file = File::open(barcode_file)?;
    let reader = io::BufReader::new(GzDecoder::new(file));

    Ok(reader.lines().filter_map(|line| line.ok()).collect())
}

#[pyclass(frozen)]
struct BarcodeSet {
    symspell: SymSpell,
    #[pyo3(get)]
    max_dist: usize,
    #[pyo3(get)]
    split_length: usize,
    #[pyo3(get)]
    barcode_length: usize,
}

#[pymethods]
impl BarcodeSet {
    /// construct a BarcodeSet, which is a set of barcodes stored in a symspell index
    /// for fast lookup and error correction
    #[new]
    fn py_new(barcode_file: PathBuf, max_dist: usize, split_length: usize) -> PyResult<Self> {
        let builder = SymSpellBuilder::default()
            .max_dictionary_edit_distance(max_dist)
            .split_length(split_length)
            .build();

        if let Ok(mut symspell) = builder {
            info!("Reading barcodes from {}", barcode_file.display());
            if let Ok(barcodes) = read_barcodes(barcode_file) {
                info!("Loading barcodes");
                let barcode_length: HashSet<_> = barcodes.iter().map(|bc| bc.len()).collect();
                if barcode_length.len() != 1 {
                    return Err(PyValueError::new_err(
                        "Found barcodes with multiple lengths",
                    ));
                }

                for bc in barcodes.iter() {
                    symspell.create_dictionary_entry(bc);
                }

                debug!("Built SymSpell index with {} barcodes", barcodes.len());
                let barcode_length = *barcode_length.iter().next().unwrap();
                Ok(BarcodeSet {
                    symspell,
                    max_dist,
                    split_length,
                    barcode_length,
                })
            } else {
                Err(PyIOError::new_err("Error reading barcode file"))
            }
        } else {
            Err(PyRuntimeError::new_err("Error building symspell"))
        }
    }

    /// Looks up a single word and returns all the closest suggestions (i.e. all words
    /// in the collection at the best distance), or an empty list if none are found.
    fn lookup(&self, query: &str) -> PyResult<Vec<(String, String, usize)>> {
        trace!("Searching for {}", query);
        let suggestions = self.symspell.lookup(query, self.max_dist);

        Ok(suggestions
            .iter()
            .cloned()
            .map(|s| (s.term, s.query, s.distance))
            .collect())
    }

    /// Looks up a batch of related strings to see if together they match to a single
    /// word. Returns all matches at the minimum distance, or an empty list.
    fn lookup_batch(&self, queries: HashSet<&str>) -> PyResult<Vec<(String, String, usize)>> {
        trace!("Searching for {} queries", queries.len());

        let suggestions = self.symspell.exact_lookup_batch(&queries);
        if !suggestions.is_empty() {
            return Ok(suggestions
                .iter()
                .map(|s| (s.term.clone(), s.term.clone(), s.distance))
                .collect());
        }

        let suggestions: Vec<_> = queries
            .iter()
            .flat_map(|q| self.symspell.lookup(q, self.max_dist))
            .collect();

        if suggestions.is_empty() {
            return Ok(Vec::new());
        }

        let min_dist = suggestions.iter().map(|s| s.distance).min().unwrap();
        let suggestions: HashSet<_> = suggestions
            .iter()
            .filter(|s| s.distance == min_dist)
            .cloned()
            .map(|s| (s.term, s.query, s.distance))
            .collect();

        Ok(suggestions.iter().cloned().collect())
    }

    /// Takes a string and look up all substrings that might plausibly be in the barcode
    /// set. This is based on max edit distance and barcode length
    fn lookup_substrings(&self, query: &str) -> PyResult<Vec<(String, String, usize)>> {
        let max_dist = self.max_dist as usize;
        if query.len() < (self.barcode_length - max_dist) {
            return Ok(Vec::new());
        }
        let mut queries = HashSet::new();

        for i in 0..(query.len() - self.barcode_length + 2 * max_dist) {
            for j in 0..(2 * max_dist + 1) {
                let k = i + j + self.barcode_length - max_dist;
                if k <= query.len() {
                    queries.insert(&query[i..k]);
                }
            }
        }

        self.lookup_batch(queries)
    }
}

/// A Python module implemented in Rust.
#[pymodule]
fn bouncer(_py: Python, m: &PyModule) -> PyResult<()> {
    pyo3_log::init();

    m.add_class::<BarcodeSet>()?;
    Ok(())
}
