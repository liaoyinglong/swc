//// [templateStringWithEmbeddedInOperator.ts]
"abc".concat("hi" in {
    hi: 10,
    hello: 20
}, "def");
