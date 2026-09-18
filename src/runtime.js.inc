"use strict";
const __mars = (() => {
  const data = new WeakMap(), restrictions = new WeakMap(), frozen = new WeakSet();
  let segmenter;
  function graphemes(value) {
    if (typeof Intl.Segmenter !== 'function') throw Error('Marslang Unicode strings require Intl.Segmenter support in Node');
    segmenter ??= new Intl.Segmenter('und', {granularity:'grapheme'});
    return Array.from(segmenter.segment(value), part => part.segment);
  }
  class OutOfBoundsError extends Error {
    constructor(message) { super(message); this.name = 'OutOfBoundsError'; }
  }
  function sliceIndex(value) {
    if (typeof value === 'bigint') {
      if (value < 0n || value > BigInt(Number.MAX_SAFE_INTEGER)) throw new OutOfBoundsError('slice index out of bounds');
      return Number(value);
    }
    if (typeof value !== 'number' || !Number.isFinite(value) || !Number.isInteger(value)) {
      throw TypeError('slice positions and lengths must be integers');
    }
    if (value < 0 || !Number.isSafeInteger(value)) throw new OutOfBoundsError('slice index out of bounds');
    return value;
  }
  function sliceItems(items, args, byLength) {
    if (args.length < 2 || args.length > 3) throw TypeError('slicing requires two arguments and an optional reverse boolean');
    const start = sliceIndex(args[0]), second = sliceIndex(args[1]);
    const reverse = args.length === 3 ? args[2] : false;
    if (typeof reverse !== 'boolean') throw TypeError('slice reverse must be a boolean');
    const size = items.length;
    if (start > size || (byLength ? second > size - start : second < start || second > size)) {
      throw new OutOfBoundsError('slice extends outside the available range');
    }
    const end = byLength ? start + second : second;
    return reverse ? items.slice(size - end, size - start) : items.slice(start, end);
  }
  const object = v => v !== null && typeof v === 'object';
  const mutable = v => { if (frozen.has(v)) throw Error('cannot mutate fixed value'); };
  const truth = v => v !== false && v !== null && v !== 0 && v !== 0n;
  const types = s => {
    let depth = 0, start = 0, parts = [];
    for (let i = 0; i < s.length; i++) {
      if (s[i] === '[') depth++;
      if (s[i] === ']') depth--;
      if (s[i] === ',' && depth === 0) { parts.push(s.slice(start, i)); start = i + 1; }
    }
    parts.push(s.slice(start)); return parts;
  };
  function typed(v, type) {
    type = type.replace(/\s/g, '');
    if (type.startsWith('[') && type.endsWith(']')) {
      for (const option of types(type.slice(1, -1))) {
        try { return typed(v, option); } catch (_) {}
      }
      throw TypeError('value does not match ' + type);
    }
    if (type === 'int') {
      if (typeof v !== 'number' || !Number.isInteger(v)) throw TypeError('expected int');
      if (v < -2147483648 || v > 2147483647) throw RangeError('int overflow');
      return v;
    }
    if (type === 'longint') {
      if (typeof v === 'number' && Number.isSafeInteger(v)) v = BigInt(v);
      if (typeof v !== 'bigint') throw TypeError('expected exact longint');
      if (v < -(1n << 63n) || v > (1n << 63n) - 1n) throw RangeError('longint overflow');
      return v;
    }
    if (type === 'float') { if (typeof v !== 'number') throw TypeError('expected float'); return v; }
    if (type === 'string') { if (typeof v !== 'string') throw TypeError('expected string'); return v; }
    const match = /^(array|set|pair|map|dict)(?:\[(.*)\])?$/.exec(type);
    if (match) {
      const kind = match[1] === 'dict' ? 'map' : match[1];
      if (!object(v) || data.get(v)?.kind !== kind) throw TypeError('expected ' + kind);
      if (match[2]) {
        const args = types(match[2]);
        if (args.length !== (kind === 'map' || kind === 'pair' ? 2 : 1)) throw TypeError('wrong type arity: ' + type);
        const d = data.get(v);
        if (kind === 'map') { for (const [k, value] of d.items) { typed(k, args[0]); typed(value, args[1]); } }
        else if (kind === 'pair') { typed(v.first, args[0]); typed(v.second, args[1]); }
        else { for (const value of d.items) typed(value, args[0]); }
        const existing = restrictions.get(v) || []; existing.push(args); restrictions.set(v, existing);
      }
      return v;
    }
    if (object(v) && v.constructor?.name === type) return v;
    throw TypeError('unknown or mismatched type ' + type);
  }
  function check(v, value, part = 0) {
    for (const args of restrictions.get(v) || []) value = typed(value, args[part]);
    return value;
  }
  class Arr {
    constructor(items) { data.set(this, {kind:'array', items}); }
    len() { return data.get(this).items.length; }
    is_empty() { return this.len() === 0; }
    has(v) { return data.get(this).items.some(x => x === v); }
    iget(i) { this.index(i); return data.get(this).items[i]; }
    index(i) { if (!Number.isInteger(i) || i < 0 || i >= this.len()) throw RangeError('array index out of bounds'); }
    add(v) { mutable(this); data.get(this).items.push(check(this,v)); return this; }
    pop() { mutable(this); return data.get(this).items.shift() ?? null; }
    lpop() { mutable(this); return data.get(this).items.pop() ?? null; }
    iremove(i) { mutable(this); this.index(i); return data.get(this).items.splice(i,1)[0]; }
    get(v) { return data.get(this).items.indexOf(v); }
    rget(v) { return data.get(this).items.lastIndexOf(v); }
    remove(v) { mutable(this); const i = this.get(v); return i < 0 ? null : this.iremove(i); }
    modify(i,v) { mutable(this); this.index(i); data.get(this).items[i] = check(this,v); return this; }
    change(a,b) { mutable(this); b = check(this,b); const d = data.get(this); d.items = d.items.map(x => x === a ? b : x); return this; }
    sort() { mutable(this); data.get(this).items.sort((a,b) => a < b ? 1 : a > b ? -1 : 0); return this; }
    asort() { mutable(this); data.get(this).items.sort((a,b) => a < b ? -1 : a > b ? 1 : 0); return this; }
    rev() { mutable(this); data.get(this).items.reverse(); return this; }
    reverse() { return this.rev(); }
    slice(...args) { return retain(this, new Arr(sliceItems(data.get(this).items,args,false))); }
    lenslice(...args) { return retain(this, new Arr(sliceItems(data.get(this).items,args,true))); }
    [Symbol.iterator]() { return data.get(this).items[Symbol.iterator](); }
  }
  class MSet {
    constructor(items) { data.set(this, {kind:'set', items:new Set(items)}); }
    len() { return data.get(this).items.size; }
    is_empty() { return this.len() === 0; }
    has(v) { return data.get(this).items.has(v); }
    add(v) { mutable(this); data.get(this).items.add(check(this,v)); return this; }
    remove(v) { mutable(this); return data.get(this).items.delete(v); }
    [Symbol.iterator]() { return data.get(this).items[Symbol.iterator](); }
  }
  class MMap {
    constructor() { data.set(this, {kind:'map', items:new Map()}); }
    len() { return data.get(this).items.size; }
    is_empty() { return this.len() === 0; }
    has(k) { return data.get(this).items.has(k); }
    get(k) { const d = data.get(this).items; return d.has(k) ? d.get(k) : null; }
    set(k,v) { mutable(this); data.get(this).items.set(check(this,k,0),check(this,v,1)); return this; }
    remove(k) { mutable(this); return data.get(this).items.delete(k); }
    keys() { return new Arr([...data.get(this).items.keys()]); }
    values() { return new Arr([...data.get(this).items.values()]); }
    [Symbol.iterator]() { return data.get(this).items.keys(); }
  }
  function pair(a,b) { const v = {first:a, second:b}; data.set(v,{kind:'pair'}); return v; }
  function retain(a,b) { if (restrictions.has(a)) restrictions.set(b, restrictions.get(a).map(x => [...x])); return b; }
  function freeze(v, seen = new WeakSet()) {
    if (!object(v) || seen.has(v)) return v;
    seen.add(v); frozen.add(v);
    const d = data.get(v);
    if (d?.kind === 'map') { for (const [k,value] of d.items) { freeze(k,seen); freeze(value,seen); } }
    else if (d && d.kind !== 'pair') { for (const value of d.items) freeze(value,seen); }
    else { for (const value of Object.values(v)) freeze(value,seen); }
    return Object.freeze(v);
  }
  function copy(v, seen = new WeakMap()) {
    if (!object(v)) return v;
    if (seen.has(v)) return seen.get(v);
    const d = data.get(v);
    let result;
    if (d?.kind === 'array') result = new Arr([]);
    else if (d?.kind === 'set') result = new MSet([]);
    else if (d?.kind === 'map') result = new MMap();
    else if (d?.kind === 'pair') result = pair(null,null);
    else result = Object.create(Object.getPrototypeOf(v));
    seen.set(v,result);
    if (d?.kind === 'map') { for (const [k,value] of d.items) result.set(copy(k,seen),copy(value,seen)); }
    else if (d && d.kind !== 'pair') { for (const value of d.items) result.add(copy(value,seen)); }
    else { for (const [k,value] of Object.entries(v)) result[k] = copy(value,seen); }
    return retain(v,result);
  }
  function setfield(v,k,value) {
    mutable(v);
    if (data.get(v)?.kind === 'pair') {
      if (k !== 'first' && k !== 'second') throw Error('invalid pair field');
      value = check(v,value,k === 'first' ? 0 : 1);
    }
    v[k] = value; return value;
  }
  function bin(op,a,b,allowFloat = false) {
    if (op === '==') return a === b;
    if (op === '!=') return a !== b;
    if (['<','<=','>','>='].includes(op)) {
      if (typeof a !== typeof b) throw TypeError('comparison requires matching types');
      return op === '<' ? a < b : op === '<=' ? a <= b : op === '>' ? a > b : a >= b;
    }
    if (op === '+' && typeof a === 'string' && typeof b === 'string') return a+b;
    if (!['number','bigint'].includes(typeof a) || !['number','bigint'].includes(typeof b)) throw TypeError('arithmetic requires numbers');
    if (typeof a === 'bigint' && typeof b === 'number' && Number.isSafeInteger(b)) b = BigInt(b);
    if (typeof b === 'bigint' && typeof a === 'number' && Number.isSafeInteger(a)) a = BigInt(a);
    if (typeof a !== typeof b) throw TypeError('mixed longint/float arithmetic');
    if ((op === '/' || op === '%') && (b === 0 || b === 0n)) throw RangeError('division by zero');
    const result = op === '+' ? a+b : op === '-' ? a-b : op === '*' ? a*b : op === '/' ? a/b : op === '%' ? a%b : a**b;
    if (typeof result === 'bigint') typed(result,'longint');
    if (!allowFloat && typeof result === 'number' && Number.isSafeInteger(a) && Number.isSafeInteger(b) && Number.isInteger(result) && !Number.isSafeInteger(result)) throw RangeError('unsafe integer arithmetic; use longint');
    return result;
  }
  return {
    typed, freeze, copy, setfield, truth, bin,
    length: (value,...args) => {
      if (typeof value !== 'string') return value.len(...args);
      if (args.length) throw TypeError('string.len() takes no arguments');
      return graphemes(value).length;
    },
    reverse: (value,...args) => {
      if (typeof value !== 'string') return value.reverse(...args);
      if (args.length) throw TypeError('string.reverse() takes no arguments');
      return graphemes(value).reverse().join('');
    },
    slice: (value,...args) => {
      if (typeof value !== 'string') return value.slice(...args);
      return sliceItems(graphemes(value),args,false).join('');
    },
    lenslice: (value,...args) => {
      if (typeof value !== 'string') return value.lenslice(...args);
      return sliceItems(graphemes(value),args,true).join('');
    },
    floatbin: (op,a,b) => {
      if (typeof a !== 'number' || typeof b !== 'number') throw TypeError('float arithmetic requires number operands');
      return bin(op,a,b,true);
    },
    unary: (op,v) => {
      if (typeof v !== 'number' && typeof v !== 'bigint') throw TypeError('unary arithmetic requires a number');
      return op === '-' ? -v : v;
    },
    iter: v => {
      if (typeof v === 'string') return graphemes(v);
      if (data.has(v) && data.get(v).kind !== 'pair') return [...v];
      throw TypeError('expected an iterable array, set, map, or string');
    },
    repeat: n => { if (!Number.isSafeInteger(n) || n < 0) throw RangeError('repeat count must be a nonnegative integer'); return n; },
    out: (...xs) => console.log(...xs.map(x => typeof x === 'bigint' ? String(x) : x)),
    slout: (...xs) => process.stdout.write(xs.map(String).join('')),
    in: () => require('fs').readFileSync(0,'utf8'),
    inln: (() => { let lines, i=0; return () => { lines ??= require('fs').readFileSync(0,'utf8').split(/\r?\n/); return lines[i++] ?? ''; }; })(),
    arr: (...xs) => new Arr(xs), a: (...xs) => new Arr(xs),
    set: (...xs) => new MSet(xs), s: (...xs) => new MSet(xs), pair, p: pair,
    map: () => new MMap(), dict: () => new MMap(),
    int: v => typed(Number(v),'int'), longint: v => typed(typeof v === 'string' ? BigInt(v) : v,'longint'),
    float: v => Number(v), string: v => String(v),
  };
})();
