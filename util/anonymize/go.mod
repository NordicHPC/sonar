module anonymize

go 1.26.8

replace github.com/NordicHPC/sonar/util/common => ../common

replace github.com/NordicHPC/sonar/util/formats => ../formats

require github.com/NordicHPC/sonar/util/common v0.0.0-00010101000000-000000000000

require github.com/NordicHPC/sonar/util/formats v0.0.0-00010101000000-000000000000
