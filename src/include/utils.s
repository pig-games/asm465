#importonce

.cpu _45gs02

.macro setJSRAddress(addr_jsr, addr_calc) {
        lda #<addr_calc
        sta addr_jsr + 1
        lda #>addr_calc
        sta addr_jsr + 2
}


.macro setBasePage(addr) {
        lda #>addr
        tab
}

.macro setBasePagePC() {
    .if((* & $00ff) > 0) {
        *= (* & $ff00) + $100
    } else {
        *= *    
    }
}

