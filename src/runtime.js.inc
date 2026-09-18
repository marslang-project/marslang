"use strict";
const __mars = (() => {
  const data = new WeakMap(), restrictions = new WeakMap(), frozen = new WeakSet();
  // Integer numbers and bigints already carry distinct host representations.
  // Float values need an immutable box to retain their kind even for 1.0 or 0.0.
  const floats = new WeakMap();
  class FloatValue {
    constructor(value) { floats.set(this,value); Object.freeze(this); }
    valueOf() { return floats.get(this); }
    toString() { return String(floats.get(this)); }
  }
  const isFloat = v => v !== null && typeof v === 'object' && floats.has(v);
  const raw = v => isFloat(v) ? floats.get(v) : v;
  const float = v => new FloatValue(Number(raw(v)));
  const numberKind = v => isFloat(v) ? 'float' : typeof v === 'bigint' ? 'longint' : typeof v === 'number' ? 'int' : null;
  const equal = (a,b) => raw(a) === raw(b);
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
    value = raw(value);
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
  const object = v => v !== null && typeof v === 'object' && !isFloat(v);
  const mutable = v => { if (frozen.has(v)) throw Error('cannot mutate fixed value'); };
  const truth = v => { v = raw(v); return v !== false && v !== null && v !== 0 && v !== 0n; };
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
      const options = types(type.slice(1, -1)), kind = numberKind(v);
      // Prefer the actual numeric kind before trying compatible conversions.
      if (kind && options.includes(kind)) return typed(v,kind);
      for (const option of options) {
        try { return typed(v, option); } catch (_) {}
      }
      throw TypeError('value does not match ' + type);
    }
    if (type === 'int') {
      v = raw(v);
      if (typeof v !== 'number' || !Number.isInteger(v)) throw TypeError('expected int');
      if (v < -2147483648 || v > 2147483647) throw RangeError('int overflow');
      return v;
    }
    if (type === 'longint') {
      v = raw(v);
      if (typeof v === 'number' && Number.isSafeInteger(v)) v = BigInt(v);
      if (typeof v !== 'bigint') throw TypeError('expected exact longint');
      if (v < -(1n << 63n) || v > (1n << 63n) - 1n) throw RangeError('longint overflow');
      return v;
    }
    if (type === 'float') { if (typeof raw(v) !== 'number') throw TypeError('expected float'); return isFloat(v) ? v : float(v); }
    if (type === 'string') { if (typeof v !== 'string') throw TypeError('expected string'); return v; }
    const match = /^(array|set|pair|map|dict)(?:\[(.*)\])?$/.exec(type);
    if (match) {
      const kind = match[1] === 'dict' ? 'map' : match[1];
      if (!object(v) || data.get(v)?.kind !== kind) throw TypeError('expected ' + kind);
      if (match[2]) {
        const args = types(match[2]);
        if (args.length !== (kind === 'map' || kind === 'pair' ? 2 : 1)) throw TypeError('wrong type arity: ' + type);
        const d = data.get(v);
        const validate = (value, type) => {
          const converted=typed(value,type);
          if(frozen.has(v) && numberKind(converted)!==numberKind(value)) mutable(v);
        };
        if (kind === 'map') { for (const [k, value] of d.items.values()) { validate(k, args[0]); validate(value, args[1]); } }
        else if (kind === 'pair') { validate(v.first, args[0]); validate(v.second, args[1]); }
        else { for (const value of kind === 'set' ? d.items.values() : d.items) validate(value, args[0]); }
        const existing = restrictions.get(v) || []; existing.push(args); restrictions.set(v, existing);
        if (kind === 'array') d.items = d.items.map(value => check(v,value));
        else if (kind === 'set') d.items = new Map([...d.items.values()].map(value => { value=check(v,value); return [raw(value),value]; }));
        else if (kind === 'map') d.items = new Map([...d.items.values()].map(([k,value]) => { k=check(v,k,0); return [raw(k),[k,check(v,value,1)]]; }));
        else if (kind === 'pair') {
          const first=check(v,v.first,0), second=check(v,v.second,1);
          if(first!==v.first) { mutable(v); v.first=first; }
          if(second!==v.second) { mutable(v); v.second=second; }
        }
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
    has(v) { return data.get(this).items.some(x => equal(x,v)); }
    iget(i) { this.index(i); return data.get(this).items[raw(i)]; }
    index(i) { i=raw(i); if (!Number.isInteger(i) || i < 0 || i >= this.len()) throw RangeError('array index out of bounds'); }
    add(v) { mutable(this); data.get(this).items.push(check(this,v)); return this; }
    pop() { mutable(this); return data.get(this).items.shift() ?? null; }
    lpop() { mutable(this); return data.get(this).items.pop() ?? null; }
    iremove(i) { mutable(this); this.index(i); return data.get(this).items.splice(i,1)[0]; }
    get(v) { return data.get(this).items.findIndex(x => equal(x,v)); }
    rget(v) { const xs=data.get(this).items; for(let i=xs.length-1;i>=0;i--) if(equal(xs[i],v)) return i; return -1; }
    remove(v) { mutable(this); const i = this.get(v); return i < 0 ? null : this.iremove(i); }
    modify(i,v) { mutable(this); this.index(i); data.get(this).items[i] = check(this,v); return this; }
    change(a,b) { mutable(this); b = check(this,b); const d = data.get(this); d.items = d.items.map(x => equal(x,a) ? b : x); return this; }
    sort() { mutable(this); data.get(this).items.sort((a,b) => a < b ? 1 : a > b ? -1 : 0); return this; }
    asort() { mutable(this); data.get(this).items.sort((a,b) => a < b ? -1 : a > b ? 1 : 0); return this; }
    rev() { mutable(this); data.get(this).items.reverse(); return this; }
    reverse() { return this.rev(); }
    slice(...args) { return retain(this, new Arr(sliceItems(data.get(this).items,args,false))); }
    lenslice(...args) { return retain(this, new Arr(sliceItems(data.get(this).items,args,true))); }
    [Symbol.iterator]() { return data.get(this).items[Symbol.iterator](); }
  }
  class MSet {
    constructor(items) { data.set(this, {kind:'set', items:new Map()}); for(const item of items) this.add(item); }
    len() { return data.get(this).items.size; }
    is_empty() { return this.len() === 0; }
    has(v) { return data.get(this).items.has(raw(v)); }
    add(v) { mutable(this); v=check(this,v); const items=data.get(this).items; if(!items.has(raw(v))) items.set(raw(v),v); return this; }
    remove(v) { mutable(this); return data.get(this).items.delete(raw(v)); }
    [Symbol.iterator]() { return data.get(this).items.values(); }
  }
  class MMap {
    constructor() { data.set(this, {kind:'map', items:new Map()}); }
    len() { return data.get(this).items.size; }
    is_empty() { return this.len() === 0; }
    has(k) { return data.get(this).items.has(raw(k)); }
    get(k) { const entry=data.get(this).items.get(raw(k)); return entry ? entry[1] : null; }
    set(k,v) { mutable(this); k=check(this,k,0); v=check(this,v,1); const items=data.get(this).items; const old=items.get(raw(k)); items.set(raw(k),[old ? old[0] : k,v]); return this; }
    remove(k) { mutable(this); return data.get(this).items.delete(raw(k)); }
    keys() { return new Arr([...data.get(this).items.values()].map(entry => entry[0])); }
    values() { return new Arr([...data.get(this).items.values()].map(entry => entry[1])); }
    *[Symbol.iterator]() { for(const [k] of data.get(this).items.values()) yield k; }
  }
  function pair(a,b) { const v = {first:a, second:b}; data.set(v,{kind:'pair'}); return v; }
  function retain(a,b) { if (restrictions.has(a)) restrictions.set(b, restrictions.get(a).map(x => [...x])); return b; }
  function freeze(v, seen = new WeakSet()) {
    if (!object(v) || seen.has(v)) return v;
    seen.add(v); frozen.add(v);
    const d = data.get(v);
    if (d?.kind === 'map') { for (const [k,value] of d.items.values()) { freeze(k,seen); freeze(value,seen); } }
    else if (d && d.kind !== 'pair') { for (const value of v) freeze(value,seen); }
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
    if (d?.kind === 'map') { for (const [k,value] of d.items.values()) result.set(copy(k,seen),copy(value,seen)); }
    else if (d && d.kind !== 'pair') { for (const value of v) result.add(copy(value,seen)); }
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
  function bin(op,a,b) {
    if (op === '==') return equal(a,b);
    if (op === '!=') return !equal(a,b);
    const floating = isFloat(a) || isFloat(b);
    a=raw(a); b=raw(b);
    if (['<','<=','>','>='].includes(op)) {
      if (typeof a !== typeof b) throw TypeError('comparison requires matching types');
      return op === '<' ? a < b : op === '<=' ? a <= b : op === '>' ? a > b : a >= b;
    }
    if (op === '+' && typeof a === 'string' && typeof b === 'string') return a+b;
    if (!['number','bigint'].includes(typeof a) || !['number','bigint'].includes(typeof b)) throw TypeError('arithmetic requires numbers');
    if (floating && (typeof a === 'bigint' || typeof b === 'bigint')) throw TypeError('mixed longint/float arithmetic');
    if (typeof a === 'bigint' && typeof b === 'number' && Number.isSafeInteger(b)) b = BigInt(b);
    if (typeof b === 'bigint' && typeof a === 'number' && Number.isSafeInteger(a)) a = BigInt(a);
    if (typeof a !== typeof b) throw TypeError('mixed longint/float arithmetic');
    if ((op === '/' || op === '%') && (b === 0 || b === 0n)) throw RangeError('division by zero');
    const result = op === '+' ? a+b : op === '-' ? a-b : op === '*' ? a*b : op === '/' ? a/b : op === '%' ? a%b : a**b;
    if (typeof result === 'bigint') typed(result,'longint');
    if (!floating && typeof result === 'number' && Number.isSafeInteger(a) && Number.isSafeInteger(b) && Number.isInteger(result) && !Number.isSafeInteger(result)) throw RangeError('unsafe integer arithmetic; use longint');
    if (!floating && typeof result === 'number' && Number.isInteger(result)) typed(result,'int');
    return typeof result === 'number' && (floating || !Number.isInteger(result)) ? float(result) : result;
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
    unary: (op,v) => {
      const floating = isFloat(v); v=raw(v);
      if (typeof v !== 'number' && typeof v !== 'bigint') throw TypeError('unary arithmetic requires a number');
      const result = op === '-' ? -v : v;
      if (!floating) typed(result,typeof result === 'bigint' ? 'longint' : 'int');
      return floating ? float(result) : result;
    },
    iter: v => {
      if (typeof v === 'string') return graphemes(v);
      if (data.has(v) && data.get(v).kind !== 'pair') return [...v];
      throw TypeError('expected an iterable array, set, map, or string');
    },
    repeat: n => { n=raw(n); if (!Number.isSafeInteger(n) || n < 0) throw RangeError('repeat count must be a nonnegative integer'); return n; },
    out: (...xs) => console.log(...xs.map(x => typeof x === 'bigint' ? String(x) : raw(x))),
    slout: (...xs) => process.stdout.write(xs.map(String).join('')),
    in: () => require('fs').readFileSync(0,'utf8'),
    inln: (() => { let lines, i=0; return () => { lines ??= require('fs').readFileSync(0,'utf8').split(/\r?\n/); return lines[i++] ?? ''; }; })(),
    arr: (...xs) => new Arr(xs), a: (...xs) => new Arr(xs),
    set: (...xs) => new MSet(xs), s: (...xs) => new MSet(xs), pair, p: pair,
    map: () => new MMap(), dict: () => new MMap(),
    int: v => typed(Number(v),'int'), longint: v => typed(typeof v === 'string' ? BigInt(v) : v,'longint'),
    float, string: v => String(v),
    mathFunction: (name,fn,arity) => (...args) => {
      if(args.length !== arity) throw TypeError('std.math.'+name+' expects '+arity+' arguments');
      const kind=numberKind(args[0]);
      if(!kind || args.some(v => numberKind(v)!==kind)) throw TypeError('std.math.'+name+' requires matching numeric types');
      args=args.map(v => typed(v,kind));
      if(kind==='float' && args.some(v => !Number.isFinite(raw(v)))) throw RangeError('std.math requires finite float values');
      if(name==='clamp' && raw(args[1])>raw(args[2])) throw RangeError('std.math.clamp lower bound exceeds upper bound');
      if(name==='abs' && ((kind==='int' && args[0]===-2147483648) || (kind==='longint' && args[0]===-9223372036854775808n))) throw RangeError(kind+' overflow');
      return typed(fn(...args),kind);
    },
  };
})();
