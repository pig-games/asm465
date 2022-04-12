.cpu _45gs02


.macro setJSRAddress(addr_jsr, addr_calc) {
        lda #<addr_calc
        sta addr_jsr + 1
        lda #>addr_calc
        sta addr_jsr + 2
}
