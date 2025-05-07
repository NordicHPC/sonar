module rebuild-jobs

go 1.23.0

require (
	github.com/NordicHPC/sonar/util/formats v0.0.0-00010101000000-000000000000
)

replace github.com/NordicHPC/sonar/util/formats => ../formats
