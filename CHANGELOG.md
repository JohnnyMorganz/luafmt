# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
### Added
- CLI will now look for `stylua.toml` and its hidden counterpart, `.stylua.toml`. ([#145](https://github.com/JohnnyMorganz/StyLua/issues/145))
- Added CLI flag `--search-parent-directories`. If enabled, we will look in parent directories for a configuration file, or look in `$XDG_CONFIG_HOME` or `$XDG_CONFIG_HOME/stylua`. ([#127](https://github.com/JohnnyMorganz/StyLua/issues/127), [#146](https://github.com/JohnnyMorganz/StyLua/issues/146))
- Updated full-moon: Added support for typed variadics under the Luau feature flag
- Will now hang on equality operators within binary expressions, if over width.

### Changed
- Long prefix expressions which are hangable and go over the line limit (e.g. `("foooo" .. "barrrrrrr" .. "bazzzzzz"):format(...)`) will now hang multiline ([#139](https://github.com/JohnnyMorganz/StyLua/issues/139))
- Changed formatting for assignments. We will now try all tactics then determine the best one. Multiple assignments will now no longer attempt to hang a single expression first - we will hang the whole punctuated list. ([#157](https://github.com/JohnnyMorganz/StyLua/issues/157))
- Function calls with single arguments are now possible to be expanded. This will allow the call to be expanded if the line goes over budget. ([[#156](https://github.com/JohnnyMorganz/StyLua/issues/156)])
- StyLua will now firstly prefer hanging long arguments to function calls to try and fit under the width, before expanding them multiline. ([#159](https://github.com/JohnnyMorganz/StyLua/issues/159))
- When hanging a binary expression, previously, we would always hang the "root" node of AST BinExp tree. Now we will check to see if is necessary (we are over width) before hanging ([#163](https://github.com/JohnnyMorganz/StyLua/issues/163))
- Function definitions (both normal and anonymous) with an empty body will now be kept on a single line. This is common for noop functions `local function noop() end`. ([#161](https://github.com/JohnnyMorganz/StyLua/issues/161#issuecomment-843596022))

### Fixed
- Fixed 1 or 2 digit numerical escapes being incorrectly removed
- Fixed whitespace being lost before a multiline comment. We will now preserve a single space (e.g. `local test  --[[foo]] = true` -> `local test --[[foo]] = true`) ([#136](https://github.com/JohnnyMorganz/StyLua/issues/136))
- Fixed the double formatting of a hanging call chain when it was being assigned to a variable causing it to be incorrectly formatted ([#151](https://github.com/JohnnyMorganz/StyLua/issues/151))
- Fixed leading comments to a binop in a hanging expression being lost ([#154](https://github.com/JohnnyMorganz/StyLua/issues/154#issuecomment-841703038))
- Fixed mistransformation of comments leading the RHS of a hanging binop. They are now moved to before the binop ([#154](https://github.com/JohnnyMorganz/StyLua/issues/154))

## [0.8.1] - 2021-04-30
### Fixed
- Fixed bug where a hanging expression inside of parentheses would lead to function arguments being incorrectly formatted with a trailing comma - leading to a syntax error

## [0.8.0] - 2021-04-30
### Added
- Parentheses around conditions are now removed, as they are not required in Lua. `if (foo and (not bar or baz)) then ... end` turns to `if foo and (not bar or baz) then ... end`
- Long multi-variable assignments which surpass the column width, even when hanging on the equals token, will now hang on multiple lines.

### Changed
- Changed the heursitics for when parentheses are removed around expressions. Parentheses will now never be removed around a function call prefix (e.g. `("hello"):len()`)
- Changed formatting for comma-separated lists. Previously, we would buffer the comments to the end of the list, but now we keep the comments next to where they original were.
- Improved contextual formatting informattion when formatting deep in the AST. We can now better determine how much space is left on the current line, before we need to change formatting
- Improved formatting of function declarations. It will now properly take into account the amount of space left on the column width.
- Improve formatting for assignments with expressions such as function calls. The whole assignment is now taken into account, so we can better determine whether to split the expression.

### Fixed
- Fixed trailing whitespace remaining on the last item of a multiline table (which was expanded from a singleline one)

## [0.7.1] - 2021-04-19
### Fixed
- Fixed parentheses around a table being incorrectly removed leading to a syntax error, such as in `({}):foo()`

## [0.7.0] - 2021-04-13
### Added
- Added hanging for chained function calls. See [#109](https://github.com/JohnnyMorganz/StyLua/issues/109)
- Long function definitions (normally with parameters containing types and a return type) will now be split across multiple lines if they surpass the column limit

### Changed
- Further improvements to the way binary expressions are hung on new lines

### Fixed
- Fixed trailing comments at the end of multiline tables being lost
- Fixed panic "stmt trailing comments not implemented" occuring due to incomplete function
- Fixed trailing comments after semicolons at the end of last statements being lost when formatting
- Fixed function parameters collapsing when there is a comments at the end of function parameters, where the last parameter has a type specifier
- Fixed comments at the end of tables being indented one extra level
- Fixed trailing comments within if-elseif-else blocks not being correctly indented.
- Fixed `do` in a `while ... do` statement not correctly indented when the condition spans multiple lines
- Fixed multiline parameters for a function definition inside of an indent block (e.g. a table) not being correctly indented

## [0.6.0] - 2021-03-27
### Added
- Added support for creating new `Config` structs when using StyLua as a library
- Added configuration for quote style. There are four quote style options - `AutoPreferDouble`, `AutoPreferSingle`, `ForceDouble` and `ForceSingle`.
For the auto styles, we will prefer the quote type specified, but fall back to the opposite if it means there are fewer escapes. For the
force styles, we will always use the quote type specified.
- StyLua will now error when unknown fields are found in the configuration `stylua.toml` file
- Long lines of assignments, where the expressions aren't hangable, will now be put onto a newline, where a newline is created after the equal sign, and the expressions indented.
- Added initial support for **Lua 5.2** syntax. StyLua can now format code containing `goto`s and labels. See [#87](https://github.com/JohnnyMorganz/StyLua/issues/87) to track further support for Lua 5.2 syntax.

### Changed
- Function call heuristic have been further improve to decide when to expand the function call arguments onto multiple lines.
- StyLua now allows some arguments after a multiline table before forcing expansion. This makes sense for something like `setmetatable({ ... }, class)`, where
`{ ... }` is a multiline table, but we don't want to expand onto multiple lines. StyLua will not allow a mixture of multiline tables and small identifiers in between
(e.g. `call({ ... }, foo, { ... })`), in order to improve readability.
- Empty newlines at the start and end of a block will now be removed as they are unnecessary
- Changed the default quote style from `ForceDouble` to `AutoPreferDouble`. We will now default to swapping quote type if it will reduce the number of escapes.

### Fixed
- Fixed tables with internal comments (and no fields) incorrectly collapsing to a single line
- Fixed parentheses being incorrectly removed around a BinOp where first value was a UnOp
- Fixed indentation of leading comments bound to the end brace of a multiline table
- Fixed LastStmt (return/break etc.) still being formatted when it wasn't defined inside the range
- Fixed hanging expressions which are inside function calls being indented unnecessarily by one extra level
- Fixed parentheses being incorrectly removed around a function definition, which may be called like `(function() ... end)()`
- Fixed some string escapes being incorrectly deemed as unnecessary
- Fixed trailing comments after semicolons at the end of statements being lost when formatting
- Fixed formatting issues in relation to newline and whitespace when using range formatting.
- Fixed empty tables taking 2 formatting passes to format properly

## [0.5.0] - 2021-02-24
### Added
- Added support for removing excess parentheses around expressions.
e.g. `print((x))` will be formatted to `print(x)`, as the parentheses are unnecessary. We also consider cases
where parentheses should not be removed, e.g. `print((x()))` - removing the parentheses changes the meaning of the code.
- Added formatting of BinOp expressions within function calls. If there is a long expression as a function argument and it contains binops, it will now span multiple lines
- Added a `column_width` setting, which is used to guide when StyLua should wrap lines. It defaults to `120`.
- Added support for formatting ranges. You can now specificy ranges using ``--range-start <num>`` and ``--range-end <num>`` (both optional, and both inclusive).
If a range is provided, only statements within the range will be formatted. Currently only supports ranges containing whole statements, and is not more granular.
- Added support for ignore comments. If the line before a statement begins with the comment `-- stylua: ignore`, then the statement will be ignored during formatting.
This currently only supports ignoring statement-level nodes

### Changed
- Improved CLI `--check` output. We now use a more detailed output which should help in determining diffs
- Improved calculations in places to determine when to wrap lines

### Fixed
- Fixed an expression ending with an UnOp (e.g. `#foo`) and a trailing comment forcing an unnecessary hanging expression
- Fixed loss of comments trailing punctuation within function parameters
- Comments within function parameters now force the parameter to go mutliline, fixing syntax errors created from previous formatting
- Fixed incorrect indentation of body of expressions spanning multiple lines (e.g. anonymous functions/tables) when the expression is part of a hanging binop
- Fixed incorrect formatting of multiple long comma-separated assignment/returns causing the comma to be placed onto a new line

## [0.4.1] - 2021-02-05
### Fixed
- Fixed function calls being incorrectly expanded due to a comment within the arguments.
We will now only check for leading/trailing comments for argument expressions to see if we need to keep it expanded or not.

## [0.4.0] - 2021-02-05
### Added
- Added formatting for number literals which begin with a decimal. For consistency, a "0" will be prepended (i.e. `.5` turns to `0.5`)
- Long expressions in a return statement will now hang onto multiple lines if necessary
- StyLua will now handle expressions in parentheses if they are long, by breaking them down further.
- Added support for ambiguous syntax. StyLua will now keep the semicolon and format as required

### Fixed
- Fixed "then" and "do" tokens not being correctly indented when if-then and while-do statements are pushed onto multiple lines
- Fixed incorrect newline formatting when a return type is present for an anonymous function in Luau
- Fixed multiline expressions where the binop has a trailing comment being incorrectly formatted, breaking code
- Fixed a trailing comment at the end of a whole binop expression unnecessarily forcing a hanging expression

## [0.3.0] - 2021-01-15
### Added
- StyLua will now test escapes of characters other than quotes in strings to see if they are unnecessary and remove them if so
- Adds wrapping for large expressions to push them onto multiple lines. Statements with line of longer than 120 characters will trigger expression wrapping where possible.
The expression will be split at its Binary Operators, excluding relational operators.

### Fixed
- Fixed `.styluaignore` file extension matching not working due to the default override glob
- Cleaned up descriptions of options when running `stylua --help`
- Fixed issue with `stylua.toml` requiring a complete configuration file with all options set
- Fixed issue with escapes unrelated to quotes inside of strings not being preserved
- Fixed incorrect formatting when trailing comments are present in function arguments and other locations.
In function arguments, it will remain expanded if there is a comment present. Similarly, comments are now preserved in punctuated sequencues.

## [0.2.1] - 2021-01-03
### Fixed
- Fixed `until` token in a repeat block not being correctly indented
- Fixed regression causing the first and last item of an expanded table to not be correctly indented

## [0.2.0] - 2020-12-31
### Changed
- Changed heuristics for expanding function arguments. StyLua will now check through the arguments and look out for expanded tables
or anonymous functions, and if found, will not expand the function call. However, if there are any other type of expression mixed between,
then the function call will remain expanded.
- Change internals of the formatter by reducing amount of cloning of AST nodes. Improves performance by 22%

## [0.1.0] - 2020-12-30
### Added
- StyLua will now take into account if a table was originally expanded onto multiple lines. If so, StyLua won't attempt to collapse it
- Added support for reading in from stdin for the CLI, use `stylua -` to make StyLua read from stdin, and a formatted output will be written to stdout
- Added `--check` command line flag. If enabled, then StyLua will check through the files and emit a diff for files with incorrect formatting, exiting with status code 1. StyLua will not modifiy files
- Renamed CLI argument `--pattern` to `--glob` (with short form `-g`). `--glob` can now accept multiple globs.
For example, using `stylua -g *.lua -g !*.spec.lua .` will format all Lua files apart from `.spec.lua` test files.
- Added support for parsing a `.styluaignore` file, which follows a similar structure to `.gitignore` files. Any patterns matched inside of this file will be ignored.

### Changed
- Changed when a table will expand onto new lines. It will now expand after 80 characters have been exceeded, and takes indent level into account

## [0.1.0-alpha.3] - 2020-12-26
### Changed
- Changed the default value of `indent_width` to 4
- Function calls with a single argument will no longer wrap the argument onto a new line. This is subject to change.

### Fixed
- Fixed a new line being added after the `until` token in a repeat block. The new line is now added at the end of the until expression.
- Fixed comments not being preserved within multiline tables
- Fixed trailing comma being added after comments in multiline tables
- Fixed escaping of double-quotes inside of strings being repeated
- Fixed long tables for types collapsing onto a single line for Luau formatting
- Fixed incorrect comment wrapping at the beginning of multiline tables
- Fixed start brace of multiline comments not having correct indentation
- Fixed comments having incorrect indentation when bound to the `end` token at the end of a block.

## [0.1.0-alpha.2] - 2020-12-22
### Added
- Single quote escapes are now removed from string literals if present when converting to double-quoted strings

### Changed
- If there is a single argument in a function call, and it is either a table or anonymous function, the relevant start/end tokens are no longer pushed onto new lines
- Comments are now left completely unformatted, apart from trimming trailing whitespace at the end of single-line comments

### Fixed
- Double quotes are now escaped when converting from single quote to double quote strings

## [0.1.0-alpha] - 2020-12-22
Initial alpha release