         lda #147
         jsr $ffd2

         sei

jumpback = $0140
         lda #<jumpback
         sta $0318
         lda #>jumpback
         sta $0319


