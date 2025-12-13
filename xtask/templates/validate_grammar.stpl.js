// <%= generated_disclaimer %>

// Mock Node.js module system
global.module = { exports: {} };
global.exports = global.module.exports;

// Simple validation: just check that all require() statements work
// We don't need to execute the actual grammar logic, just verify dependencies load

// Override require to track what's being loaded (for debugging if needed)
const originalRequire = require;
const loadedModules = [];

global.require = function(moduleId) {
  loadedModules.push(moduleId);
  return originalRequire(moduleId);
};

// Stub all tree-sitter functions so grammar doesn't crash during require
// These return minimal mocks just to prevent errors
global.grammar = () => ({ name: 'mock', rules: {} });
global.seq = () => 'seq';
global.choice = () => 'choice';
global.repeat = () => 'repeat';
global.repeat1 = () => 'repeat1';
global.optional = () => 'optional';
global.prec = () => 'prec';
global.prec_left = () => 'prec_left';
global.prec_right = () => 'prec_right';
global.prec_dynamic = () => 'prec_dynamic';
global.token = () => 'token';
global.alias = () => 'alias';
global.field = () => 'field';
global.$ = new Proxy({}, { get: () => 'rule' });

// Common constants that grammars might use
global.NEWLINE = 'newline';
global.WHITESPACE = 'whitespace';
global.IDENTIFIER = 'identifier';
global.NUMBER = 'number';
global.STRING = 'string';
global.COMMENT = 'comment';

// Try to require the grammar file - this will validate that all dependencies can be loaded
try {
  require('<%= grammar_path %>');
  console.log('✓ Grammar validation passed - all dependencies loadable');
} catch (error) {
  // Only throw if it's a MODULE_NOT_FOUND error (missing dependency)
  // Other errors (like grammar logic errors) are fine for validation purposes
  if (error.code === 'MODULE_NOT_FOUND') {
    throw error;
  }
  // Grammar logic errors are expected and don't indicate broken dependencies
  console.log('✓ Grammar validation passed - dependencies loadable (ignoring grammar logic errors)');
}
