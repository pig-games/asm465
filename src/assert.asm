abra2    .segment

\3       jsr sprint
;         .byte 5
         .null \1, ": "
         #cprl 129,"Success!"
         jmp e_\3
\2       jsr sprint
;         .byte 5
         .null \1, ": "
         #cprl 153,"Fail!"
e_\3
         .endm









