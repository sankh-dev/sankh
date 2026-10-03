# Assertions and chaining

A request file checks its response with `@expect`, saves values from it with
`@capture`, and later requests in the same run use those values as ordinary
`$VARIABLES`. This page walks through all three. Every operator and capture
source is listed in [Collection format](format.md).

## Asserting a response

### Status

```bash
# @expect status 201
# @expect status 2xx
# @expect status 200|204
```

Without any `@expect status`, a request passes on any `2xx` status. Tests that
expect an error need an explicit expectation, e.g. `@expect status 404`.

### JSON body

`@expect json <jq> <op> <value>` runs a jq expression against the response body
and compares the result. Given this response:

```json
{"id": 7, "name": "Rex", "status": "available", "tags": ["good-boy"], "items": [1, 2]}
```

all of these pass:

```bash
# @expect json .name == "Rex"
# @expect json .id > 0
# @expect json .id exists
# @expect json .status matches "^(available|pending|sold)$"
# @expect json .tags contains "good-boy"
# @expect json .items | length > 0
```

- The operator is `==`, not `=`.
- The value is a JSON literal: write `"Rex"`, not `Rex`. Numbers, `true`,
  `false`, `null`, arrays and objects work too.
- The expression may contain pipes and operators of its own. Sankh splits the
  line from the right, so only the last operator and value are the comparison.
- Strings may embed variables: `@expect json .owner == "$USER_ID"`.
- Every `@expect` line is evaluated, and all must pass.

### Arrays

Given this response:

```json
{"data": [{"id": "u_1", "name": "Ann"}, {"id": "u_2", "name": "Bob"}]}
```

```bash
# First and last item
# @expect json .data[0].id == "u_1"
# @expect json .data[-1].id == "u_2"

# Number of items
# @expect json .data | length == 2

# Some item matches
# @expect json .data | map(.id) contains "u_2"
# @expect json any(.data[]; .name == "Bob") == true

# Every item matches
# @expect json all(.data[]; .id | startswith("u_")) == true
```

When the response body is itself an array, start with `.[0]` or `.[]`
instead of a field name:

```json
[{"id": "PROD001", "price": 79.99, "specifications": {"color": "Black"}}]
```

```bash
# @expect json .[0].id == "PROD001"
# @expect json . | length > 0
# @expect json map(.id) contains "PROD001"
# @expect json all(.[]; .id | startswith("PROD")) == true
# @expect json .[0].specifications.color == "Black"
# @expect json .[0].price > 0
# @capture PRODUCT_ID=.[0].id
```

An expression must produce exactly one value, so `.data[].id == "u_1"` fails
with `` `.data[].id` produced 2 values; expected one ``. Wrap the iteration in
`map`, `any`, `all` or `[...]` to turn it into a single value.

## Capturing values

`@capture NAME=<source>` stores a value from the response:

```bash
# @capture PET_ID=.id                     # jq expression on the JSON body
# @capture FIRST_USER_ID=.data[0].id      # works on arrays too
# @capture REQUEST_ID=header X-Request-Id # response header, case-insensitive
# @capture CREATED=status                 # HTTP status code
```

- `NAME` uses uppercase letters, digits and `_`.
- Captures run only when every assertion in the file passed.
- A `null` result, a missing value or several values fail the request.
- Strings are stored without quotes. Objects and arrays are stored as compact
  JSON, so `@capture USERS=.data` stores
  `[{"id":"u_1","name":"Ann"},{"id":"u_2","name":"Bob"}]`.
- Captured values live only for the current run and are never written to disk.

## Chaining a flow

The bundled [petstore example](https://github.com/sankh-dev/sankh/tree/main/examples/petstore)
logs in, creates a pet, reads it back, deletes it and checks that it is gone.
Each step feeds the next:

```text
auth/01-login.sh          captures TOKEN
pets/02-create.sh         uses $TOKEN, captures PET_ID and REQUEST_ID
pets/03-get.sh            uses $TOKEN and $PET_ID
pets/04-delete.sh         uses $TOKEN and $PET_ID
pets/05-get-deleted.sh    uses $TOKEN and $PET_ID, expects 404
```

`auth/01-login.sh` checks that a token came back and captures it:

```bash
#!/usr/bin/env bash
# @name Log in
# @expect status 200
# @expect json .token exists
# @capture TOKEN=.token
curl -sS "$BASE_URL/login" \
  -H 'Content-Type: application/json' \
  -d "{\"username\":\"$PET_USER\",\"password\":\"$PET_PASSWORD\"}"
```

`pets/02-create.sh` sends the token, checks the new pet, and captures its id and
a response header:

```bash
#!/usr/bin/env bash
# @name Create pet
# @expect status 201
# @expect json .name == "Rex"
# @expect json .tags contains "good-boy"
# @capture PET_ID=.id
# @capture REQUEST_ID=header X-Request-Id
curl -sS "$BASE_URL/pets" \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"name":"Rex","tags":["good-boy"]}'
```

`pets/03-get.sh` uses the captured id in the URL and in an assertion. The id
is a number in the JSON, and captures are always text, so `tostring` makes the
two comparable:

```bash
#!/usr/bin/env bash
# @name Get pet
# @expect status 200
# @expect json .id | tostring == "$PET_ID"
curl -sS "$BASE_URL/pets/$PET_ID" \
  -H "Authorization: Bearer $TOKEN"
```

`pets/04-delete.sh` expects `204`, and `pets/05-get-deleted.sh` is a negative
test:

```bash
#!/usr/bin/env bash
# @name Deleted pet is gone
# @expect status 404
# @expect json .error == "not found"
curl -sS "$BASE_URL/pets/$PET_ID" \
  -H "Authorization: Bearer $TOKEN"
```

Run the whole flow (start the mock first with `cargo run --example petstore_mock`):

```bash
sankh run examples/petstore
```

```text
sankh · petstore · env dev (6 requests)
✓ Log in  200  1ms  auth/01-login.sh
    captured TOKEN=***
✓ List pets  200  1ms  pets/01-list.sh
✓ Create pet  201  1ms  pets/02-create.sh
    captured PET_ID=1
    captured REQUEST_ID=req-1
✓ Get pet  200  1ms  pets/03-get.sh
✓ Delete pet  204  1ms  pets/04-delete.sh
✓ Deleted pet is gone  404  1ms  pets/05-get-deleted.sh

6 passed  220ms
```

`TOKEN` is shown as `***` because Sankh masks variables whose names look like
secrets (see [Trust and secrets](trust-secrets.md)).

## Ordering and running part of a flow

- Captures flow in run order: numeric prefixes (`01-`, `02-`) first, then
  names. Use `[order]` in `sankh.toml` to override it (see
  [Collection format](format.md#ordering)).
- Without `--bail`, a run keeps going after a failure, and later steps run
  with the variable unset. Use `sankh run --bail` to stop at the first failure.
- Running a single file starts with no captures, so Sankh warns:

  ```text
  warning: $PET_ID is not set (define it in an env file, .env.local or the process environment)
  ```

  Pass the values through the process environment instead:

  ```bash
  TOKEN=... PET_ID=1 sankh run examples/petstore/pets/03-get.sh
  ```

- In the [web UI](serve.md) and the [desktop app](desktop.md), captured values
  are kept per collection and environment for the session, so you can run the
  login request once and then run other requests one at a time. **Clear
  captures** forgets them.

## Where a variable comes from

Lowest to highest priority:

1. `environments/<name>.env`
2. `.env.local` (gitignored)
3. The process environment (CI secrets)
4. Values captured earlier in the run

A capture therefore overrides a variable of the same name from any env file or
the process environment.
