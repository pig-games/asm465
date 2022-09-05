# asm465

Asm465 is an, in-development, native editor/assembler for the Commodore 64/128.
The number in the name literally means for 65(02).

This editor/assembler has some very specific design goals.

1. Run natively on the Commodore 64/128.
2. Support the full 6502 instruction set and addressing modes.
3. UI/Editor
   1. Fast native editing, specifically tailored and optimised for assember editing
   2. Optional live per source line indication of: Address, Instruction cycle counts (also for scope)
   3. Syntax highlighting
   4. Auto complete
   5. Multiple files open at the same time
   6. Tiled panels in stead of floating windows
4. Support modern (assembler) features like:
   1. Modules
   2. (Nested) labels with scope/namespace
   3. Rich set of pseudo operations like (.if, .for, etc).
   4. Macro's
5. Background processing
   1. Resolving label addresses
   2. Resolving line addresses
   3. Resolving instruction cycle counts
   4. Assembling
6. File import/export
   1. ASCII <-> Asm465 formatted files
7. Be fully self hosted as soon as possible.
  
The notation is chosen to be close to that of Kick Assembler, but not needing to be fully compatible. 
It will possibly make sense to make some Win/Mac/Linux command line tools
for translating back and forth between the two.

Disclaimer: I'm aware this is quite a lofty set of goals, but development will be very incremental. Knowing 
where you want to end up however, does help a lot not making design decisions early on that will make it hard
to reach the final set of goals in the future.
