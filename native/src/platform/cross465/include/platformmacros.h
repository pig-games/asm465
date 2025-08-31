; Ultimate64 platform macros (6502-safe equivalents to M65 ones)
PLATFORMMACROS :?= false
.if !PLATFORMMACROS
PLATFORMMACROS := true

.include "platformdefs.h"

bra .function label
    jmp label
.endfunction

; ---- push/pop “quad” (A,X,Y only on 6502) ---------------------------------
phq .function
    sta AStore
    pha         ; save A
    txa
    pha         ; save X via A
    tya
    pha         ; save Y via A
    lda AStore
.endfunction

plq .function
    pla         ; restore Y into A
    tay         ; store into Y
    pla         ; restore X into A
    tax         ; store into X
    pla         ; restore A
.endfunction

plx .function
    sta AStore
    pla         ; restore X into A
    tax         ; store into X
    lda AStore
.endfunction

ply .function
    sta AStore
    pla         ; restore Y into A
    tay         ; store into Y
    lda AStore
.endfunction

phx .function
    sta AStore
    txa         ; save X into A
    pha         ; save A
    lda AStore
.endfunction

phy .function
    sta AStore
    tya         ; save Y into A
    pha         ; save A
    lda AStore
.endfunction

adq .function ptr
    clc
    adc \ptr
    sta \ptr
    bcc *+2
    inc \ptr+1
    txa
    adc \ptr
    sta \ptr
    bcc *+2
    inc \ptr+1
    tya
    adc \ptr
    sta \ptr
    bcc *+2
    inc \ptr+1
    rts
.endfunction


.endif
