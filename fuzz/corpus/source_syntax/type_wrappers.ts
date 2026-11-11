(require as Function)('./a' as const);
(require satisfies Function)('./b' satisfies string);
require!('./c'!);
(<Function>require)(<string>'./d');
(require<unknown>)('./e');
import('./f' as string);
require(variable as './ignored-type');
(other as 'require')('./ignored-callee');
