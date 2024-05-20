<div align="center">
    <img src="./assets/splint.png" width="300px"/>
    <div>
    <a href="https://crates.io/crates/splint"><img alt="Crates.io Version" src="https://img.shields.io/crates/v/splint?style=for-the-badge"></a>
    <a href="https://docs.rs/splint"><img alt="docs.rs" src="https://img.shields.io/docsrs/splint?style=for-the-badge"></a>    
    </div>
    <br/>
    <h1>Custom Rust Linting</h1>
</div>

### Cli Usage
```bash
# Install Splint
cargo install splint

# Run splint
splint -r <rules.json> src/**/*.rs # Splint only works on rust files
```

### Rules
The following rule looks for a sequence of `.unwrap()` anywhere in the file.  
You don't need to worry about whitespace, as it uses a parsed stream of tokens from proc_macro2.
```json
{
    "rules": {
        "Disallow Unwrap": {
        /* The name of your lint  */                        "name": "Disallow Unwrap",
        /* Reasoning for the lint */                        "description": "`.unwrap()` should be discouraged where possible, as it leads to less than usefull panics.",
        /* (optional) Describe a fix or alternative */       "help": "Favour '?' for Results, or handling with unwrap_or(). At the least give some diagnostics with .expect()",
        /* (optional) Link to more information */            "more": "https://doc.rust-lang.org/std/result/enum.Result.html#method.unwrap",
        /* Whether or not this lint should panic*/          "fail": false,
        /* The inclusive range highlighted */               "range": {
        /* Pattern Index */                                     "start": 0,
        /* Pattern Index */                                     "end": 3
                                                            },
        /* Type/Value matching */                           "pattern": [
        /* Type is one of Puct/Ident/Delim */                   ["Punct", "."],
        /* Where Punctuation handles punctuation, */            ["Ident", "unwrap"],
        /* Delim brackets, and Ident other strings/ */          ["Delim", "("],
        /* Regex in value is defined by surrounding '/' */      ["Delim", ")"]
        /* The value can also be `null` */                  ]
        }
    }
}
```

### Thanks
- #### 🩷 [proc_macro2](https://docs.rs/proc-macro2) & [syn](https://docs.rs/syn) for the brains of parsing
- #### 🩷 [miette](https://docs.rs/miette/) for the gorgeous error handling 