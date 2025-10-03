;; This is meant to approximate what we find in clang-format.txt.
;; This is supposed to be "stroustrup + simple tweaks".

((c-mode . ((indent-tabs-mode . nil)
	    (fill-column . 100)
	    (c-basic-offset . 4)
	    (c-comment-only-line-offset . 0)
	    (c-file-offsets .
	     ((case-label . 0)
	      (statement-case-intro . 4)
	      (statement-block-intro . +)
	      (substatement-open . 0)
	      (substatement-label . 0)
	      (label . 0)
	      (brace-list-intro first
				c-lineup-2nd-brace-entry-in-arglist
				c-lineup-class-decl-init-+ +)
	      (statement-cont . +))))))
