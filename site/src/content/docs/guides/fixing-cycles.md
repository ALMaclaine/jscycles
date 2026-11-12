---
title: Fixing cycles
description: The common shapes of a circular import, and how to break each one.
---

Every cycle is broken by removing one import from the loop. The work is
choosing which one, and moving code so the remaining imports all point the
same way: from code that does more toward code that does less.

## Find the edge to cut

Start from the loop jscycles prints:

```text
✗ @acme/core: 1 cycle
  src/index.ts -> src/session.ts -> src/user.ts -> src/index.ts
```

Read each arrow as "imports". Look for the arrow that points the wrong way:
a low-level module (a helper, a model, a type) importing a high-level one (a
barrel, a service, an entry point). That is usually the import to remove.

After the fix, run jscycles again. A tangle of files can contain several
loops and jscycles shows one at a time, so the next one may appear in the
same package.

## Imports through a barrel file

The most common cause. A barrel `index.ts` re-exports a package's modules, and
a module inside the package imports from the barrel:

```ts title="src/index.ts"
export * from './format';
export * from './user';
```

```ts title="src/user.ts"
import { formatName } from './index'; // goes through the barrel
```

`export * from './user'` makes the barrel depend on `user.ts`, so `user.ts`
importing the barrel closes a loop, even though it only needs `format.ts`.

**Fix:** inside a package, import the module that defines the value. Keep the
barrel for code outside the package.

```ts title="src/user.ts" del={1} ins={2}
import { formatName } from './index';
import { formatName } from './format';
```

## Two modules that need each other

`order.ts` uses `customer.ts`, and `customer.ts` uses a helper from
`order.ts`:

```ts title="src/order.ts"
import { Customer } from './customer';
export const total = (prices: number[]) => prices.reduce((a, b) => a + b, 0);
export class Order { constructor(public customer: Customer, public prices: number[]) {} }
```

```ts title="src/customer.ts"
import { total } from './order';
export class Customer {
  orders: number[][] = [];
  lifetimeValue() { return total(this.orders.map(total)); }
}
```

**Fix:** move what both need into a third module that imports neither.

```ts title="src/pricing.ts"
export const total = (prices: number[]) => prices.reduce((a, b) => a + b, 0);
```

Now `order.ts` and `customer.ts` both import `pricing.ts`, and nothing imports
back.

## A cycle made only of types

Two modules refer to each other's types:

```ts title="src/user.ts"
import { Session } from './session';
export interface User { sessions: Session[] }
```

```ts title="src/session.ts"
import { User } from './user';
export interface Session { user: User }
```

TypeScript erases these imports, so the loop never runs. If only runtime
cycles matter to you, mark the imports as type-only and run jscycles with
`--skip-type-imports` (or set `skip_type_imports: true` in `jscycles.yaml`):

```ts title="src/session.ts" del={1} ins={2}
import { User } from './user';
import type { User } from './user';
```

`import { type User }` is not enough: under `verbatimModuleSyntax` it still
loads `./user` at runtime, so it still counts.

If you want type cycles gone too, move the shared types into a module of their
own, such as `types.ts`, that imports nothing from the modules using it.

## A base class that knows its subclasses

A base class creates or checks for its subclasses:

```ts title="src/animal.ts"
import { Dog } from './dog';
export class Animal {
  static create(kind: string) { return kind === 'dog' ? new Dog() : new Animal(); }
}
```

```ts title="src/dog.ts"
import { Animal } from './animal';
export class Dog extends Animal {}
```

This is the loop most likely to crash: if `animal.ts` loads first, `dog.ts`
runs `extends Animal` before `Animal` exists and throws
`ReferenceError: Cannot access 'Animal' before initialization`.

**Fix:** move the code that knows about subclasses out of the base class.

```ts title="src/create-animal.ts"
import { Animal } from './animal';
import { Dog } from './dog';
export const createAnimal = (kind: string) => (kind === 'dog' ? new Dog() : new Animal());
```

## A low-level module calling back into a high-level one

A logger, store or HTTP client imports the application code that uses it,
for example to read configuration or report errors:

```ts title="src/http.ts"
import { reportError } from './app';
export async function get(url: string) {
  try { return await fetch(url); } catch (error) { reportError(error); throw error; }
}
```

**Fix:** invert the dependency. Let the high-level code hand the low-level
module what it needs, as an argument or by registering it once at startup:

```ts title="src/http.ts"
let onError: (error: unknown) => void = () => {};
export const setErrorHandler = (handler: typeof onError) => { onError = handler; };
export async function get(url: string) {
  try { return await fetch(url); } catch (error) { onError(error); throw error; }
}
```

```ts title="src/app.ts"
import { setErrorHandler } from './http';
setErrorHandler(reportError);
```

## Cycles between packages

A package cycle means two workspace packages import each other by name. Run
`jscycles --outer -v` to see which files create each edge, then:

- **Move shared code down.** If `@acme/utils` imports `@acme/core` only for a
  type or a small helper, move it into `@acme/utils` or into a new package
  both can depend on, such as `@acme/types`.
- **Invert one dependency.** Pass a callback or register an implementation
  instead of importing it, as above.
- **Merge the packages** if they always change together and cannot be used
  apart.

A package cycle that goes only through `import type` disappears with
`--skip-type-imports`.

## Why not import lazily?

Moving an import into a function, as `require('./x')` or `await import('./x')`,
often stops a crash, because the other module has finished loading by the time
the function runs. The loop is still there, and jscycles still reports it,
since it counts dynamic `import()` and `require()` with a literal path. Treat
it as a stopgap, and leave a comment saying which cycle it hides.

To leave a file out of the scan while you fix the rest, add it to
[`ignore`](../../reference/configuration/#defaults-and-packages) for its
package.
