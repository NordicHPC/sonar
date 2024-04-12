BEGIN {
  # Arguably keeping all the times is unnecessary, we just need min
  # and max, but it's OK for debugging.
  delete job[0]			# job[pid][...] is cputime for jobs
  delete bash[0]		# bash[pid][...] is cputime for interactive bash shells
  delete run[0]			# run[pid][...] is cputime for run_test.sh shells
  WALLTIME=strtonum(WALLTIME)	# Execution time in seconds of tomost sonar-job
  NUMJOBS=strtonum(NUMJOBS)     # Number of jobs we should see
}

{
  time=number_field($0, "cputime_sec")
  pid=number_field($0, "pid")
  cmd=string_field($0, "cmd")
  switch (cmd) {
    case /.*sonar-job/:
      if (isarray(job[pid]))
	job[pid][length(job[pid])]=time
      else
	job[pid][0]=time
      break
    case /.*bash/:
      if (isarray(bash[pid]))
	bash[pid][length(bash[pid])]=time
      else
	bash[pid][0]=time
      break
    case /.*run_test/:
      if (isarray(run[pid]))
	run[pid][length(run[pid])]=time
      else
	run[pid][0]=time
      break
  }
}

END {
  fail=0
  if (length(job) != NUMJOBS) {
    print "Wrong number of jobs, expected " NUMJOBS " got " length(job)
    fail=1
  }
  for (pid in job) {
    diff=job[pid][length(job[pid])-1] - job[pid][0]
    if (diff < WALLTIME/2) {
      print "Job " pid ": Job cost " diff " is too small for wall time " WALLTIME
      for ( x in job[pid] ) {
        print "  " job[pid][x]
      }
      fail=1
    }
  }

  for (pid in bash) {
    diff=bash[pid][length(bash[pid])-1] - bash[pid][0]
    if (diff > 2) {
      print "Bash " pid ": Shell cost " diff " is too high"
      for ( x in bash[pid] ) {
        print "  " bash[pid][x]
      }
      fail=1
    }
  }

  if (length(run) == 0) {
    print "No test runners!"
    fail=1
  }

  for (pid in run) {
    diff=run[pid][length(run[pid])-1] - run[pid][0]
    if (diff > 2) {
      print "Run " pid ": Shell cost " diff " is too high"
      for ( x in run[pid] ) {
        print "  " run[pid][x]
      }
      fail=1
    }
  }

  if (fail) {
    exit 1
  }
}

function number_field(input, tag,     s) {
  s = string_field(input, tag)
  if (s != "")
    return strtonum(s)
  return 0
}

function string_field(input, tag,    s, pat, ix) {
  s = ""
  pat="," tag "="
  ix=index(input, pat)
  if (ix > 0) {
    s=substr($0, ix+length(pat))
    ix=index(s, ",")
    if (ix > 0) {
      s=substr(s, 0, ix-1)
    }
  }
  return s
}

