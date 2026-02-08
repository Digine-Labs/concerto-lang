## Dynamic fault-tolerant ledger model

As we know AI agents can generate hallucinated responses. In order to make the ledger database fault tolerant we propose following schema

`Identifier`: String, Mostly a single sentence. Higher character limit but only one value.
`Keys`: Array of string, used to query values.
`Value`: String.

### Design choice

As we can let agents query through ledger database, agents can request wrong identifiers. So we have to design a similarity check algorithm for both querying with keys or identifier.

### Example

Lets assume db has following values

`Identifier`: `Uniswap contract addresses on Ethereum mainnet.`
`Keys`: [`Uniswap`, `Contract Address`, `Ethereum`, `Dex`, `Dex contracts`]
`Value`: `Pool: 0x12312313, Router: 0x456456456 ...`

`Identifier`: `AAVE contract addresses on Ethereum mainnet.`
`Keys`: [`AAVE`, `Contract Address`, `Ethereum`, `Lending`, `Money market contracts`]
`Value`: `Pool: 0x756745, MoneyMarket: 0x7890789 ...`

So in concerto we must have following functions to query ledger db

`ledger.query().from_identifier("Uniswap")` => This query returns First value because identifier sentence has a Uniswap word.

`ledger.query().from_identifier("contract addresses")` => This query returns both values because both have contract addresses words in their identifier sentences.

`ledger.query().from_key("Ethereum")` => This returns both values because both has Ethereum key

`ledger.query().from_key("Contract")` => This returns nothing because none of them has contract key. Contract Address key will not match here because keys must be exact values but case-insensitive.

`ledger.query().from_any_keys(["Dex", "Lending"])` => This returns both because both has any of those keys passed.

`ledger.query().from_exact_keys(["Dex, "Lending"])` => This returns none because any of those doesnt have both keys.

### Limitations

Identifiers are strings and can be considered as description of the value. Length of the string must be enough to store 2-3 sentences.

Keys are string arrays. You can decide the limit of array and string length depending on the runtime engine.

Values are also strings so you can decide the value limit for it. However as those responses will work with agents. The reference file paths can be stored there aswell. For example;

```
// ...
`Value`: "Read CONTRACT_ADDRESSES.md to find current mainnet addresses"
```

### Design

Ledger can be designed as a concerto library if possible.


### What Ledger is not.

- Ledger is not a tool or an mcp that will be connected to concerto runtime.
- Ledger calls must be done on concerto parts. But we can design tools that agent can response to use ledger. This is actually needed, but do not design ledger as a tool or mcp directly.
