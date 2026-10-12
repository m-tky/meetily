// Independent CPU-only runner using the upstream shared CLI/session helper.
#include "qwen3_asr.h"
#include "core/qwen3_stream_model.h"
#include "core/qwen3_stream.h"
#include <thread>
#include "gguf.h"
#include "ggml.h"
#include <chrono>
#include <fstream>
#include <iostream>
#include <map>
#include <sstream>
#include <stdexcept>
#include <sys/resource.h>
using Clock=std::chrono::steady_clock;
double seconds(Clock::time_point t){return std::chrono::duration<double>(Clock::now()-t).count();}
std::string quoted(const std::string& s){
 std::string r="\"";const char* hex="0123456789abcdef";
 for(unsigned char c:s){if(c=='"'||c=='\\'){r+='\\';r+=c;}else if(c<32){r+="\\u00";r+=hex[c>>4];r+=hex[c&15];}else r+=c;}return r+'"';
}
unsigned u16(const std::vector<unsigned char>& b,size_t p){return b.at(p)|(b.at(p+1)<<8);}
unsigned u32(const std::vector<unsigned char>& b,size_t p){return u16(b,p)|(u16(b,p+2)<<16);}
std::vector<float> wav(const std::string& path){
 std::ifstream in(path,std::ios::binary);std::vector<unsigned char>b((std::istreambuf_iterator<char>(in)),{});
 if(b.size()<44||b.size()>16*1024*1024||std::string((char*)b.data(),4)!="RIFF"||std::string((char*)b.data()+8,4)!="WAVE")throw std::runtime_error("Bad WAV");
 bool fmt=false;std::vector<float> samples;
 for(size_t p=12;p+8<=b.size();){auto n=u32(b,p+4);size_t start=p+8;if(n>b.size()-start)throw std::runtime_error("Truncated WAV");auto tag=std::string((char*)b.data()+p,4);
 if(tag=="fmt "){if(n<16||u16(b,start)!=1||u16(b,start+2)!=1||u32(b,start+4)!=16000||u16(b,start+14)!=16)throw std::runtime_error("Bad WAV format");fmt=true;}
 if(tag=="data"){if(!fmt||n%2||n>60*16000*2)throw std::runtime_error("Bad PCM");for(size_t i=0;i<n;i+=2){int s=u16(b,start+i);if(s>=32768)s-=65536;samples.push_back(s/32768.f);}break;}
 p=start+n+(n%2);
 }
 if(samples.empty())throw std::runtime_error("Empty WAV");return samples;
}
int main(int argc,char** argv){try{
 if(argc!=6)throw std::runtime_error("model threads offline inputs.tsv repeats");
 int threads=std::stoi(argv[2]),repeats=std::stoi(argv[5]);if(threads<1||threads>12||repeats<1||repeats>5||(std::string(argv[3])!="offline"&&std::string(argv[3])!="paced"&&std::string(argv[3])!="paced-reconcile"))throw std::runtime_error("Bad options");
 auto meta=gguf_init_from_file(argv[1],{true,nullptr});if(!meta)throw std::runtime_error("GGUF metadata failed");std::map<std::string,int> types;
 for(int64_t i=0;i<gguf_get_n_tensors(meta);i++)types[ggml_type_name(gguf_get_tensor_type(meta,i))]++;
 std::cerr<<"tensor_types_json={\"event\":\"tensor_types\",\"types\":{";bool comma=false;for(auto& [type,count]:types){if(comma)std::cerr<<',';comma=true;std::cerr<<quoted(type)<<':'<<count;}std::cerr<<"}}\n";gguf_free(meta);
 auto params=qwen3_asr_context_default_params();params.n_threads=threads;params.verbosity=0;params.use_gpu=false;params.flash_attn=false;
 auto t=Clock::now();auto ctx=qwen3_asr_init_from_file(argv[1],params);if(!ctx)throw std::runtime_error("Model load failed");double load=seconds(t);
 bool warm=false;std::ifstream inputs(argv[4]);if(!inputs)throw std::runtime_error("Missing TSV");std::string line;
 while(std::getline(inputs,line)){
 std::istringstream fields(line);std::string id,language,path;std::getline(fields,id,'\t');std::getline(fields,language,'\t');std::getline(fields,path);
 if(language!="Japanese"&&language!="English"&&language!="Thai")throw std::runtime_error("Invalid language");auto samples=wav(path);
 auto infer=[&](){bool ok=false;auto text=core_qwen3_stream_model::generate(ctx,false,samples,"language "+language+"<asr_text>",512,&ok);if(!ok)throw std::runtime_error("Inference failed");return text;};
 if(!warm){infer();warm=true;}
 for(int rep=0;rep<repeats;rep++){
 auto start=Clock::now();std::string text;double compute=0,first_provisional=-1;
 bool reconcile=std::string(argv[3])=="paced-reconcile";
 bool paced=std::string(argv[3])=="paced"||reconcile;
 if(!paced){text=infer();compute=seconds(start);}else{
  namespace stream=core_qwen3_stream;
  stream::State st;st.cfg.stock_qwen=true;st.cfg.unfixed_chunk_num=2;st.cfg.unfixed_token_num=5;st.cfg.force_language=stream::utf8_decode_replace(language);
  stream::Hooks hooks;
  hooks.encode=[&](const std::string& s){int n=0;auto ids=qwen3_asr_tokenize(ctx,s.c_str(),&n);if(!ids)throw std::runtime_error("Tokenizer failed");std::vector<int32_t> v(ids,ids+n);free(ids);return v;};
  hooks.decode_bytes=[&](const std::vector<int32_t>& ids){std::string s;for(auto id:ids)s+=core_bpe::token_bytes_to_utf8(qwen3_asr_token_text(ctx,id));return s;};
  hooks.generate=[&](const std::vector<float>& a,const std::string& prefix,int max_new){bool ok=false;auto s=core_qwen3_stream_model::generate(ctx,false,a,"language "+language+"<asr_text>"+prefix,max_new,&ok);if(!ok)throw std::runtime_error("Stream generation failed");return s;};
  for(size_t offset=0;offset<samples.size();offset+=32000){size_t end=std::min(samples.size(),offset+32000);std::this_thread::sleep_until(start+std::chrono::duration_cast<Clock::duration>(std::chrono::duration<double>(end/16000.)));
   auto tick=Clock::now();if(end==samples.size()&&reconcile)text=infer();else if(end==samples.size())stream::finish(st,hooks,samples.data()+offset,end-offset,512);else stream::step(st,hooks,samples.data()+offset,end-offset,32);compute+=seconds(tick);
   if(first_provisional<0&&!st.text.empty())first_provisional=seconds(start);
  }
  if(!reconcile)text=stream::utf8_encode(st.text);
 }
 double elapsed=seconds(start);rusage usage{};getrusage(RUSAGE_SELF,&usage);
 std::cout<<"{\"id\":"<<quoted(id)<<",\"language\":"<<quoted(language)<<",\"text\":"<<quoted(text)<<",\"status\":\"ok\",\"rep\":"<<rep
 <<",\"elapsed_seconds\":"<<elapsed<<",\"compute_seconds\":"<<compute<<",\"audio_seconds\":"<<samples.size()/16000.
 <<",\"first_provisional_seconds\":"<<(first_provisional>=0?std::to_string(first_provisional):"null")<<",\"end_delay_seconds\":"<<(paced?std::to_string(elapsed-samples.size()/16000.):"null")
 <<",\"load_seconds\":"<<load<<",\"peak_rss_kib\":"<<usage.ru_maxrss<<",\"first_token_seconds\":null,\"threads\":"<<threads
 <<",\"precision\":\"GGUF-Q4_K-mixed\",\"engine\":\"crisp-cpp\",\"mode\":"<<quoted(argv[3])<<",\"max_new_tokens\":512}\n"<<std::flush;
 }
 }
 qwen3_asr_free(ctx);return 0;
 }catch(const std::exception& e){std::cerr<<e.what()<<'\n';return 1;}}
