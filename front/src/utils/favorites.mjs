export function normalizeFavorites(value) {
 if (!Array.isArray(value)) return [];
 return [...new Set(value.filter(v=>typeof v==="string" && /^[a-zA-Z0-9_]{1,25}$/.test(v)).map(v=>v.toLowerCase()))];
}
