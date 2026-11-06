import subprocess,pathlib,json,time,sys
root=pathlib.Path(sys.argv[1]).resolve() if len(sys.argv)>1 else pathlib.Path('/tmp/myserv-protocols')
def docker(*args,check=True):
 return subprocess.run(['docker',*args],check=check,text=True,capture_output=True).stdout.strip()
network='myserv-test-net';names=['myserv-test-server','myserv-test-client']
try:
 docker('network','create',network)
 docker('run','-d','--name',names[0],'--network',network,'--cap-add=NET_ADMIN','--device=/dev/net/tun','-v',str(root)+':/fixture:ro','--entrypoint','sleep','myserv-awg:3.1.20260828','infinity')
 ip=json.loads(docker('inspect',names[0]))[0]['NetworkSettings']['Networks'][network]['IPAddress']
 text=(root/'awg-client').read_text()
 text='\n'.join(l for l in text.splitlines() if not l.startswith(('Address =','DNS =','MTU ='))).replace('203.0.113.10:51820',ip+':51820')
 (root/'awg-client-native').write_text(text+'\n')
 docker('run','-d','--name',names[1],'--network',network,'--cap-add=NET_ADMIN','--device=/dev/net/tun','-v',str(root)+':/fixture:ro','--entrypoint','sleep','myserv-awg:3.1.20260828','infinity')
 for name,conf,address in [(names[0],'awg-server','10.78.0.1/24'),(names[1],'awg-client-native','10.78.0.2/24')]:
  docker('exec','-d',name,'amneziawg-go','-f','awg0')
  for _ in range(30):
   result=subprocess.run(['docker','exec',name,'ip','link','show','awg0'],capture_output=True)
   if result.returncode==0:break
   time.sleep(.2)
  docker('exec',name,'awg','setconf','awg0','/fixture/'+conf)
  docker('exec',name,'ip','addr','add',address,'dev','awg0')
  docker('exec',name,'ip','link','set','mtu','1280','up','dev','awg0')
 # Send UDP over the tunnel with bash /dev/udp, then verify authenticated handshake and byte counters.
 docker('exec',names[1],'bash','-c',"printf 'myserv-tunnel-test' >/dev/udp/10.78.0.1/5555")
 time.sleep(2)
 for name in names:
  hs=docker('exec',name,'awg','show','awg0','latest-handshakes')
  assert any(int(row.split()[1])>0 for row in hs.splitlines()),'No authenticated handshake'
  tr=docker('exec',name,'awg','show','awg0','transfer')
  assert any(int(row.split()[1])>0 and int(row.split()[2])>0 for row in tr.splitlines()),'No bidirectional tunnel bytes'
 print('AWG 3.1: both official parsers accepted profiles; authenticated handshake and bidirectional encrypted transfer PASS')
finally:
 for name in names:docker('rm','-f',name,check=False)
 docker('network','rm',network,check=False)
