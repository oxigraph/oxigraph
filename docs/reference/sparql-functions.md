# SPARQL Functions Reference

Complete reference of SPARQL functions supported in Oxigraph.

## Table of Contents

- [String Functions](#string-functions)
- [Numeric Functions](#numeric-functions)
- [Date and Time Functions](#date-and-time-functions)
- [Hash Functions](#hash-functions)
- [Logical Functions](#logical-functions)
- [Type Testing Functions](#type-testing-functions)
- [Type Conversion Functions](#type-conversion-functions)
- [RDF Term Functions](#rdf-term-functions)
- [Comparison and Conditional Functions](#comparison-and-conditional-functions)

## String Functions

### STR

Converts an RDF term to a string.

**Syntax:** `STR(value)`

**Example:**
```sparql
SELECT (STR(?uri) AS ?uriString)
WHERE {
  ?uri a <http://example.com/Person> .
}
```

For IRIs, returns the IRI string. For literals, returns the lexical form without datatype or language tag.

### STRLEN

Returns the length of a string in characters.

**Syntax:** `STRLEN(string)`

**Example:**
```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?name (STRLEN(?name) AS ?length)
WHERE {
  ?person foaf:name ?name .
  FILTER(STRLEN(?name) > 5)
}
```

### SUBSTR

Extracts a substring from a string.

**Syntax:** `SUBSTR(string, startPos)` or `SUBSTR(string, startPos, length)`

**Parameters:**
- `string`: The source string
- `startPos`: Starting position (1-based, first character is position 1)
- `length`: Number of characters to extract (optional)

**Example:**
```sparql
SELECT (SUBSTR("Hello World", 1, 5) AS ?result)
WHERE {}
# Returns "Hello"

SELECT (SUBSTR("Hello World", 7) AS ?result)
WHERE {}
# Returns "World"
```

**Practical Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?person ?initials
WHERE {
  ?person ex:firstName ?first ;
          ex:lastName ?last .
  BIND(CONCAT(SUBSTR(?first, 1, 1), ". ", SUBSTR(?last, 1, 1), ".") AS ?initials)
}
```

### UCASE

Converts a string to uppercase.

**Syntax:** `UCASE(string)`

**Example:**
```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?name (UCASE(?name) AS ?upperName)
WHERE {
  ?person foaf:name ?name .
}
```

### LCASE

Converts a string to lowercase.

**Syntax:** `LCASE(string)`

**Example:**
```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?name
WHERE {
  ?person foaf:name ?name .
  FILTER(LCASE(?name) = "alice")
}
```

### STRSTARTS

Tests if a string starts with a prefix.

**Syntax:** `STRSTARTS(string, prefix)`

**Example:**
```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name
WHERE {
  ?person foaf:name ?name .
  FILTER(STRSTARTS(?name, "A"))
}
```

### STRENDS

Tests if a string ends with a suffix.

**Syntax:** `STRENDS(string, suffix)`

**Example:**
```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?email
WHERE {
  ?person foaf:mbox ?email .
  FILTER(STRENDS(STR(?email), "@example.com"))
}
```

### CONTAINS

Tests if a string contains a substring.

**Syntax:** `CONTAINS(string, substring)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?product ?description
WHERE {
  ?product ex:description ?description .
  FILTER(CONTAINS(?description, "warranty"))
}
```

### STRBEFORE

Returns the substring before the first occurrence of a separator.

**Syntax:** `STRBEFORE(string, separator)`

**Example:**
```sparql
SELECT (STRBEFORE("alice@example.com", "@") AS ?username)
WHERE {}
# Returns "alice"
```

### STRAFTER

Returns the substring after the first occurrence of a separator.

**Syntax:** `STRAFTER(string, separator)`

**Example:**
```sparql
SELECT (STRAFTER("alice@example.com", "@") AS ?domain)
WHERE {}
# Returns "example.com"
```

### CONCAT

Concatenates multiple strings.

**Syntax:** `CONCAT(string1, string2, ...)`

**Example:**
```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person (CONCAT(?firstName, " ", ?lastName) AS ?fullName)
WHERE {
  ?person foaf:firstName ?firstName ;
          foaf:lastName ?lastName .
}
```

**With More Arguments:**
```sparql
PREFIX ex: <http://example.com/>

SELECT (CONCAT(?street, ", ", ?city, ", ", ?state, " ", ?zip) AS ?address)
WHERE {
  ?location ex:street ?street ;
            ex:city ?city ;
            ex:state ?state ;
            ex:zip ?zip .
}
```

### ENCODE_FOR_URI

Encodes a string for safe use in a URI.

**Syntax:** `ENCODE_FOR_URI(string)`

**Example:**
```sparql
SELECT (ENCODE_FOR_URI("Hello World!") AS ?encoded)
WHERE {}
# Returns "Hello%20World%21"
```

**Practical Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?searchUrl
WHERE {
  ?query ex:searchTerm ?term .
  BIND(IRI(CONCAT("http://example.com/search?q=", ENCODE_FOR_URI(?term))) AS ?searchUrl)
}
```

### REPLACE

Replaces occurrences matching a regular expression.

**Syntax:** `REPLACE(string, pattern, replacement)` or `REPLACE(string, pattern, replacement, flags)`

**Parameters:**
- `flags`: Optional regex flags (i = case insensitive, m = multiline, s = dot matches newline, x = ignore whitespace)

**Example:**
```sparql
SELECT (REPLACE("Hello World", "World", "SPARQL") AS ?result)
WHERE {}
# Returns "Hello SPARQL"

SELECT (REPLACE("color: red", "colou?r", "colour", "i") AS ?result)
WHERE {}
# Returns "colour: red"
```

### REGEX

Tests if a string matches a regular expression pattern.

**Syntax:** `REGEX(string, pattern)` or `REGEX(string, pattern, flags)`

**Example:**
```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?email
WHERE {
  ?person foaf:mbox ?email .
  FILTER(REGEX(STR(?email), "^[a-z]+@example\\.com$", "i"))
}
```

**Common Patterns:**
```sparql
# Email validation
FILTER(REGEX(?email, "^[\\w.-]+@[\\w.-]+\\.[a-zA-Z]{2,}$"))

# Phone number (US)
FILTER(REGEX(?phone, "^\\d{3}-\\d{3}-\\d{4}$"))

# URL matching
FILTER(REGEX(STR(?url), "^https?://"))
```

## Numeric Functions

### ABS

Returns the absolute value.

**Syntax:** `ABS(numeric)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?transaction (ABS(?amount) AS ?absoluteAmount)
WHERE {
  ?transaction ex:amount ?amount .
}
```

### ROUND

Rounds to the nearest integer.

**Syntax:** `ROUND(numeric)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?product (ROUND(?price) AS ?roundedPrice)
WHERE {
  ?product ex:price ?price .
}
```

### CEIL

Rounds up to the nearest integer.

**Syntax:** `CEIL(numeric)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?item (CEIL(?price * 1.2) AS ?priceWithTax)
WHERE {
  ?item ex:price ?price .
}
```

### FLOOR

Rounds down to the nearest integer.

**Syntax:** `FLOOR(numeric)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?person (FLOOR(?ageInYears) AS ?age)
WHERE {
  ?person ex:ageInYears ?ageInYears .
}
```

### RAND

Returns a random number between 0 and 1.

**Syntax:** `RAND()`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?product ?name
WHERE {
  ?product ex:name ?name .
}
ORDER BY RAND()
LIMIT 10
```

Returns 10 random products.

### Arithmetic Operators

Standard arithmetic operators are supported:

```sparql
PREFIX ex: <http://example.com/>

SELECT ?item
       (?price + ?shipping AS ?total)
       (?price * 1.2 AS ?priceWithTax)
       (?price - ?discount AS ?finalPrice)
       (?total / ?quantity AS ?pricePerUnit)
WHERE {
  ?item ex:price ?price ;
        ex:shipping ?shipping ;
        ex:discount ?discount ;
        ex:quantity ?quantity .
  BIND(?price + ?shipping AS ?total)
}
```

## Date and Time Functions

### NOW

Returns the current date and time.

**Syntax:** `NOW()`

**Example:**
```sparql
SELECT (NOW() AS ?currentDateTime)
WHERE {}
```

**Practical Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?event ?title
WHERE {
  ?event ex:title ?title ;
         ex:startDate ?startDate .
  FILTER(?startDate > NOW())
}
```

Finds future events.

### YEAR

Extracts the year from a date or dateTime.

**Syntax:** `YEAR(datetime)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?person ?birthDate (YEAR(?birthDate) AS ?birthYear)
WHERE {
  ?person ex:birthDate ?birthDate .
}
```

### MONTH

Extracts the month (1-12) from a date or dateTime.

**Syntax:** `MONTH(datetime)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?event ?date
WHERE {
  ?event ex:date ?date .
  FILTER(MONTH(?date) = 12)  # December events
}
```

### DAY

Extracts the day of month (1-31) from a date or dateTime.

**Syntax:** `DAY(datetime)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?person ?name ?birthDate
WHERE {
  ?person ex:name ?name ;
          ex:birthDate ?birthDate .
  FILTER(MONTH(?birthDate) = MONTH(NOW()) && DAY(?birthDate) = DAY(NOW()))
}
```

Finds people with birthdays today.

### HOURS

Extracts the hours (0-23) from a dateTime.

**Syntax:** `HOURS(datetime)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?event ?time
WHERE {
  ?event ex:scheduledTime ?time .
  FILTER(HOURS(?time) >= 9 && HOURS(?time) < 17)
}
```

Finds events during business hours.

### MINUTES

Extracts the minutes (0-59) from a dateTime.

**Syntax:** `MINUTES(datetime)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?meeting (MINUTES(?startTime) AS ?minutesPastHour)
WHERE {
  ?meeting ex:startTime ?startTime .
}
```

### SECONDS

Extracts the seconds (0-59) from a dateTime.

**Syntax:** `SECONDS(datetime)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?event ?timestamp
WHERE {
  ?event ex:timestamp ?timestamp .
  FILTER(SECONDS(?timestamp) = 0)  # Events at the top of the minute
}
```

### TIMEZONE

Returns the timezone offset as a duration.

**Syntax:** `TIMEZONE(datetime)`

**Example:**
```sparql
SELECT (TIMEZONE(NOW()) AS ?myTimezone)
WHERE {}
```

### TZ

Returns the timezone identifier string.

**Syntax:** `TZ(datetime)`

**Example:**
```sparql
SELECT (TZ(NOW()) AS ?timezoneString)
WHERE {}
```

**Practical Date Example:**
```sparql
PREFIX ex: <http://example.com/>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

SELECT ?person ?name ?age
WHERE {
  ?person ex:name ?name ;
          ex:birthDate ?birthDate .

  BIND(YEAR(NOW()) - YEAR(?birthDate) AS ?age)

  FILTER(?age >= 18 && ?age < 65)
}
ORDER BY ?age
```

## Hash Functions

### MD5

Returns the MD5 hash of a string.

**Syntax:** `MD5(string)`

**Example:**
```sparql
SELECT (MD5("Hello World") AS ?hash)
WHERE {}
# Returns "b10a8db164e0754105b7a99be72e3fe5"
```

### SHA1

Returns the SHA-1 hash of a string.

**Syntax:** `SHA1(string)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?password (SHA1(?password) AS ?hashedPassword)
WHERE {
  ?user ex:password ?password .
}
```

### SHA256

Returns the SHA-256 hash of a string.

**Syntax:** `SHA256(string)`

**Example:**
```sparql
SELECT (SHA256("Hello World") AS ?hash)
WHERE {}
```

### SHA384

Returns the SHA-384 hash of a string.

**Syntax:** `SHA384(string)`

### SHA512

Returns the SHA-512 hash of a string.

**Syntax:** `SHA512(string)`

**Example:**
```sparql
SELECT (SHA512("sensitive data") AS ?hash)
WHERE {}
```

## Logical Functions

### IF

Returns one of two values based on a condition.

**Syntax:** `IF(condition, valueIfTrue, valueIfFalse)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?product ?price
       (IF(?price > 100, "Expensive", "Affordable") AS ?category)
WHERE {
  ?product ex:price ?price .
}
```

**Nested IF:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?student ?grade
       (IF(?grade >= 90, "A",
        IF(?grade >= 80, "B",
        IF(?grade >= 70, "C",
        IF(?grade >= 60, "D", "F")))) AS ?letterGrade)
WHERE {
  ?student ex:grade ?grade .
}
```

### COALESCE

Returns the first non-null value.

**Syntax:** `COALESCE(value1, value2, ...)`

**Example:**
```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX ex: <http://example.com/>

SELECT ?person (COALESCE(?preferredName, ?firstName, "Unknown") AS ?displayName)
WHERE {
  ?person a foaf:Person .
  OPTIONAL { ?person ex:preferredName ?preferredName }
  OPTIONAL { ?person foaf:firstName ?firstName }
}
```

### Logical Operators

```sparql
PREFIX ex: <http://example.com/>

SELECT ?item ?price ?stock
WHERE {
  ?item ex:price ?price ;
        ex:stock ?stock .

  # AND
  FILTER(?price > 10 && ?stock > 0)

  # OR
  # FILTER(?price < 5 || ?stock > 100)

  # NOT
  # FILTER(!(?price > 1000))
}
```

## Type Testing Functions

### ISIRI / ISURI

Tests if a value is an IRI/URI.

**Syntax:** `ISIRI(value)` or `ISURI(value)`

**Example:**
```sparql
SELECT ?s ?o
WHERE {
  ?s ?p ?o .
  FILTER(ISIRI(?o))
}
```

Returns triples where the object is an IRI.

### ISBLANK

Tests if a value is a blank node.

**Syntax:** `ISBLANK(value)`

**Example:**
```sparql
SELECT ?s ?o
WHERE {
  ?s ?p ?o .
  FILTER(ISBLANK(?o))
}
```

### ISLITERAL

Tests if a value is a literal.

**Syntax:** `ISLITERAL(value)`

**Example:**
```sparql
SELECT ?s ?o
WHERE {
  ?s ?p ?o .
  FILTER(ISLITERAL(?o))
}
```

### ISNUMERIC

Tests if a value is a numeric literal.

**Syntax:** `ISNUMERIC(value)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?property ?value
WHERE {
  ?item ?property ?value .
  FILTER(ISNUMERIC(?value))
}
```

### BOUND

Tests if a variable is bound.

**Syntax:** `BOUND(variable)`

**Example:**
```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name ?email
WHERE {
  ?person foaf:name ?name .
  OPTIONAL { ?person foaf:mbox ?email }
  FILTER(!BOUND(?email))
}
```

Finds people without email addresses.

## Type Conversion Functions

### STR

Converts a term to a string (see [String Functions](#str)).

### DATATYPE

Returns the datatype of a literal.

**Syntax:** `DATATYPE(literal)`

**Example:**
```sparql
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

SELECT ?value (DATATYPE(?value) AS ?type)
WHERE {
  ?s ?p ?value .
  FILTER(ISLITERAL(?value))
}
```

**Filter by Datatype:**
```sparql
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

SELECT ?s ?number
WHERE {
  ?s ?p ?number .
  FILTER(DATATYPE(?number) = xsd:integer)
}
```

### LANG

Returns the language tag of a literal.

**Syntax:** `LANG(literal)`

**Example:**
```sparql
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

SELECT ?resource ?label (LANG(?label) AS ?language)
WHERE {
  ?resource rdfs:label ?label .
  FILTER(LANG(?label) != "")
}
```

### LANGMATCHES

Tests if a language tag matches a language range.

**Syntax:** `LANGMATCHES(languageTag, languageRange)`

**Example:**
```sparql
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

SELECT ?resource ?label
WHERE {
  ?resource rdfs:label ?label .
  FILTER(LANGMATCHES(LANG(?label), "en"))
}
```

**Language Matching:**
```sparql
# Match any English variant (en, en-US, en-GB, etc.)
FILTER(LANGMATCHES(LANG(?label), "en"))

# Match any language
FILTER(LANGMATCHES(LANG(?label), "*"))

# Match specific variant
FILTER(LANG(?label) = "en-US")
```

### IRI / URI

Creates an IRI from a string.

**Syntax:** `IRI(string)` or `URI(string)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?generatedIRI
WHERE {
  ?item ex:id ?id .
  BIND(IRI(CONCAT("http://example.com/item/", ?id)) AS ?generatedIRI)
}
```

## RDF Term Functions

### BNODE

Creates a blank node.

**Syntax:** `BNODE()` or `BNODE(identifier)`

**Example:**
```sparql
# Generate fresh blank node
SELECT (BNODE() AS ?newNode)
WHERE {}

# Generate blank node with identifier
SELECT (BNODE("node1") AS ?node)
WHERE {}
```

**In CONSTRUCT:**
```sparql
PREFIX ex: <http://example.com/>

CONSTRUCT {
  ?person ex:address ?addr .
  ?addr ex:street ?street ;
        ex:city ?city .
}
WHERE {
  ?person ex:street ?street ;
          ex:city ?city .
  BIND(BNODE() AS ?addr)
}
```

### UUID

Generates a random UUID IRI.

**Syntax:** `UUID()`

**Example:**
```sparql
SELECT (UUID() AS ?newId)
WHERE {}
# Returns something like: urn:uuid:12345678-1234-5678-1234-567812345678
```

### STRUUID

Generates a random UUID string.

**Syntax:** `STRUUID()`

**Example:**
```sparql
SELECT (STRUUID() AS ?newIdString)
WHERE {}
# Returns something like: "12345678-1234-5678-1234-567812345678"
```

**Practical Example:**
```sparql
PREFIX ex: <http://example.com/>

INSERT {
  ?newItem ex:id ?id ;
           ex:name ?name ;
           ex:created ?now .
}
WHERE {
  VALUES ?name { "New Item" }
  BIND(UUID() AS ?newItem)
  BIND(STRUUID() AS ?id)
  BIND(NOW() AS ?now)
}
```

### SAMETERM

Tests if two terms are exactly the same.

**Syntax:** `SAMETERM(term1, term2)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?person1 ?person2
WHERE {
  ?person1 ex:knows ?person2 .
  FILTER(!SAMETERM(?person1, ?person2))
}
```

Different from `=` which performs value-based comparison:

```sparql
# These are different:
FILTER(?x = 1.0)         # True for 1 (integer) and 1.0 (decimal)
FILTER(SAMETERM(?x, 1.0)) # Only true for exactly 1.0
```

## Comparison and Conditional Functions

### IN

Tests if a value is in a list.

**Syntax:** `value IN (value1, value2, ...)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?product ?category
WHERE {
  ?product ex:category ?category .
  FILTER(?category IN ("Electronics", "Computers", "Phones"))
}
```

### NOT IN

Tests if a value is not in a list.

**Syntax:** `value NOT IN (value1, value2, ...)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?product ?status
WHERE {
  ?product ex:status ?status .
  FILTER(?status NOT IN ("Discontinued", "OutOfStock"))
}
```

### EXISTS

Tests if a pattern has at least one solution.

**Syntax:** `EXISTS { pattern }`

**Example:**
```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name
WHERE {
  ?person foaf:name ?name .
  FILTER EXISTS { ?person foaf:knows ?friend }
}
```

Finds people who know at least one other person.

### NOT EXISTS

Tests if a pattern has no solutions.

**Syntax:** `NOT EXISTS { pattern }`

**Example:**
```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name
WHERE {
  ?person foaf:name ?name .
  FILTER NOT EXISTS { ?person foaf:mbox ?email }
}
```

Finds people without email addresses.

## Aggregate Functions

### COUNT

Counts the number of values.

**Syntax:** `COUNT(expression)` or `COUNT(DISTINCT expression)` or `COUNT(*)`

**Example:**
```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT (COUNT(*) AS ?totalPeople)
WHERE {
  ?person a foaf:Person .
}
```

**With GROUP BY:**
```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person (COUNT(?friend) AS ?friendCount)
WHERE {
  ?person foaf:knows ?friend .
}
GROUP BY ?person
```

### SUM

Calculates the sum of numeric values.

**Syntax:** `SUM(expression)` or `SUM(DISTINCT expression)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?category (SUM(?price) AS ?totalValue)
WHERE {
  ?product ex:category ?category ;
           ex:price ?price .
}
GROUP BY ?category
```

### AVG

Calculates the average of numeric values.

**Syntax:** `AVG(expression)` or `AVG(DISTINCT expression)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?category (AVG(?rating) AS ?avgRating)
WHERE {
  ?product ex:category ?category ;
           ex:rating ?rating .
}
GROUP BY ?category
```

### MIN

Returns the minimum value.

**Syntax:** `MIN(expression)` or `MIN(DISTINCT expression)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?category (MIN(?price) AS ?minPrice)
WHERE {
  ?product ex:category ?category ;
           ex:price ?price .
}
GROUP BY ?category
```

### MAX

Returns the maximum value.

**Syntax:** `MAX(expression)` or `MAX(DISTINCT expression)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?category (MAX(?price) AS ?maxPrice)
WHERE {
  ?product ex:category ?category ;
           ex:price ?price .
}
GROUP BY ?category
```

### GROUP_CONCAT

Concatenates values into a single string.

**Syntax:** `GROUP_CONCAT(expression)` or `GROUP_CONCAT(expression; SEPARATOR="sep")`

**Example:**
```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person (GROUP_CONCAT(?friendName; SEPARATOR=", ") AS ?friends)
WHERE {
  ?person foaf:name ?name ;
          foaf:knows ?friend .
  ?friend foaf:name ?friendName .
}
GROUP BY ?person
```

### SAMPLE

Returns an arbitrary value from the group.

**Syntax:** `SAMPLE(expression)`

**Example:**
```sparql
PREFIX ex: <http://example.com/>

SELECT ?category (SAMPLE(?product) AS ?exampleProduct)
WHERE {
  ?product ex:category ?category .
}
GROUP BY ?category
```

## Extension Functions

Oxigraph supports custom extension functions. Contact the documentation for information about available extensions or how to implement your own.

## Function Reference Table

| Function | Category | Returns | Example |
|----------|----------|---------|---------|
| STR | String | String | `STR(?uri)` |
| STRLEN | String | Integer | `STRLEN("hello")` → 5 |
| SUBSTR | String | String | `SUBSTR("hello", 2, 3)` → "ell" |
| UCASE | String | String | `UCASE("hello")` → "HELLO" |
| LCASE | String | String | `LCASE("HELLO")` → "hello" |
| STRSTARTS | String | Boolean | `STRSTARTS("hello", "he")` → true |
| STRENDS | String | Boolean | `STRENDS("hello", "lo")` → true |
| CONTAINS | String | Boolean | `CONTAINS("hello", "ell")` → true |
| STRBEFORE | String | String | `STRBEFORE("a@b.com", "@")` → "a" |
| STRAFTER | String | String | `STRAFTER("a@b.com", "@")` → "b.com" |
| CONCAT | String | String | `CONCAT("a", "b")` → "ab" |
| ENCODE_FOR_URI | String | String | `ENCODE_FOR_URI("a b")` → "a%20b" |
| REPLACE | String | String | `REPLACE("abc", "b", "x")` → "axc" |
| REGEX | String | Boolean | `REGEX("abc", "^a")` → true |
| ABS | Numeric | Numeric | `ABS(-5)` → 5 |
| ROUND | Numeric | Integer | `ROUND(3.7)` → 4 |
| CEIL | Numeric | Integer | `CEIL(3.2)` → 4 |
| FLOOR | Numeric | Integer | `FLOOR(3.9)` → 3 |
| RAND | Numeric | Double | `RAND()` → 0.0-1.0 |
| NOW | DateTime | DateTime | `NOW()` → current time |
| YEAR | DateTime | Integer | `YEAR(?date)` → 2024 |
| MONTH | DateTime | Integer | `MONTH(?date)` → 1-12 |
| DAY | DateTime | Integer | `DAY(?date)` → 1-31 |
| HOURS | DateTime | Integer | `HOURS(?time)` → 0-23 |
| MINUTES | DateTime | Integer | `MINUTES(?time)` → 0-59 |
| SECONDS | DateTime | Decimal | `SECONDS(?time)` → 0-59 |
| MD5 | Hash | String | `MD5("abc")` → hash |
| SHA1 | Hash | String | `SHA1("abc")` → hash |
| SHA256 | Hash | String | `SHA256("abc")` → hash |
| SHA512 | Hash | String | `SHA512("abc")` → hash |
| IF | Logical | Value | `IF(test, true, false)` |
| COALESCE | Logical | Value | `COALESCE(?a, ?b, "default")` |
| ISIRI | Type Test | Boolean | `ISIRI(?x)` |
| ISBLANK | Type Test | Boolean | `ISBLANK(?x)` |
| ISLITERAL | Type Test | Boolean | `ISLITERAL(?x)` |
| ISNUMERIC | Type Test | Boolean | `ISNUMERIC(?x)` |
| BOUND | Type Test | Boolean | `BOUND(?x)` |
| DATATYPE | Type | IRI | `DATATYPE(?lit)` |
| LANG | Type | String | `LANG(?lit)` |
| LANGMATCHES | Type | Boolean | `LANGMATCHES("en", "en-US")` |
| IRI | RDF | IRI | `IRI("http://ex.com")` |
| BNODE | RDF | BlankNode | `BNODE()` |
| UUID | RDF | IRI | `UUID()` |
| STRUUID | RDF | String | `STRUUID()` |
| SAMETERM | Comparison | Boolean | `SAMETERM(?x, ?y)` |

## See Also

- [SPARQL Introduction](../tutorials/sparql-introduction.md)
- [Advanced SPARQL Queries](../how-to/sparql-advanced-queries.md)
- [SPARQL Updates](../how-to/sparql-updates.md)
- [SPARQL 1.1 Functions Specification](https://www.w3.org/TR/sparql11-query/#SparqlOps)
