const noAsyncEffect = {
  meta: {
    type: 'problem',
    docs: { description: 'require an explicit synchronous cleanup boundary for effects' },
  },
  create(context) {
    return {
      CallExpression(node) {
        if (
          node.callee.type === 'Identifier' &&
          node.callee.name === '$effect' &&
          node.arguments.some((argument) => argument.async)
        ) {
          context.report({
            node,
            message:
              'Do not pass an async function directly to $effect; start work synchronously and return cleanup.',
          });
        }
      },
    };
  },
};

const noLiteralError = {
  meta: { type: 'problem', docs: { description: 'keep literal UI diagnostics localizable' } },
  create(context) {
    return {
      NewExpression(node) {
        if (
          node.callee.type === 'Identifier' &&
          node.callee.name === 'Error' &&
          node.arguments[0]?.type === 'Literal' &&
          typeof node.arguments[0].value === 'string'
        ) {
          context.report({
            node,
            message: 'Use ValidationError for app-owned user-facing failures.',
          });
        }
      },
    };
  },
};

export default { rules: { 'no-async-effect': noAsyncEffect, 'no-literal-error': noLiteralError } };
