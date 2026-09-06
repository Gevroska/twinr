import {test} from "node:test";
import assert from "node:assert/strict";
import {normalizeFavorites} from "./favorites.mjs";
test("favorites are validated and case-insensitively deduplicated",()=>{
 assert.deepEqual(normalizeFavorites(["Raz404","raz404","ok_name","../bad",null,42,"a".repeat(26)]),["raz404","ok_name"]);
 assert.deepEqual(normalizeFavorites({}),[]);
});
