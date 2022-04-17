# asm465

Asm465 is an, in-development, native editor/assembler for the Mega65.
The number in the name is a combination of the cpu names: 45(gs02) and 65(02).

This editor assembler has some very specific design goals.

1. Run natively on the Mega65.
2. Support the full 45gs02 instruction set (including 65(ce)02) and addressing modes.
3. Editor
   1. Fast native 80x50 character mode editing, specifically tailored and optimised for assember editing
   2. Optional live per source line indication of: Address, Instruction cycle counts (also for scope)
   3. Syntax highlighting
   4. Auto complete
4. Support modern assembler features like:
   1. (Nested) labels with scope/namespace
   2. Rich set of pseudo operations like (.if, .for, etc).
   3. Macro's
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
