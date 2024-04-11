# WALLTIME must have the execution time in seconds

BEGIN {
  delete session[0]
  delete job[0]
  delete bash[0]
  WALLTIME=strtonum(WALLTIME)
}

# For each line, parse the cputime_sec if present and bucket the output by command
{
  time=0
  ix=index($0, ",cputime_sec=")
  if (ix > 0) {
    s=substr($0, ix+13)
    ix=index(s, ",")
    if (ix > 0) {
      s=substr(s, 0, ix-1)
    }
    time=strtonum(s)
  }
  if (index($0, ",cmd=sonar-session") > 0) {
    session[length(session)]=time
  } else if (index($0, ",cmd=sonar-job") > 0) {
    job[length(job)]=time
  } else if (index($0, ",cmd=bash") > 0) {
    ix=index($0, ",pid=")
    if (ix > 0) {
      s=substr($0, ix+5)
      ix=index(s, ",")
      if (ix > 0) {
        s=substr(s, 0, ix-1)
      }
      if (isarray(bash[s])) {
        bash[s][length(bash[s])]=time
      } else {
        bash[s][0]=time
      }
    }
  }
}

# Check that the bucket values are sane
END {
  if (length(session) == 0) {
    print "No sessions!"
    exit 1
  }
  diff=session[length(session)-1] - session[0]
  if (diff > 2) {
    print "Session cost " diff " is too high"
    for ( x in session ) {
      print "  " session[x]
    }
    exit 1
  }

  if (length(job) == 0) {
    print("no jobs!")
    exit 1
  }
  diff=job[length(job)-1] - job[0]
  if (diff < WALLTIME/2) {
    print "Job cost " diff " is too small for wall time " WALLTIME
    for ( x in job ) {
      print "  " job[x]
    }
    exit 1
  }

  for (pid in bash) {
    diff=bash[pid][length(bash[pid])-1] - bash[pid][0]
    if (diff > 2) {
      print "Bash cost " diff " is too high"
      for ( x in bash[pid] ) {
        print "  " bash[pid][x]
      }
      exit 1
    }
  }
}
