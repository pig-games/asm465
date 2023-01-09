abra2    .segment

@2       jsr sprint
         .byte 5
         .text "@1: "
         .byte 0
         #cprl 129,"Fail!"
         jmp e{CBM-@}@3
@3       jsr sprint
         .byte 5
         .text "@1: "
         .byte 0
         #cprl 153,"Success!"
e{CBM-@}@3
         .endm









