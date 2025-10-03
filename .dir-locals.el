;; This is meant to approximate what we find in clang-format.txt.

((c-mode . ((c-default-style . "stroustrup")
	    (indent-tabs-mode . nil)
	    (fill-column . 100)
	    (c-file-offsets .
	     ((case-label . 0)
	      (statement-block-intro . 4))))))
