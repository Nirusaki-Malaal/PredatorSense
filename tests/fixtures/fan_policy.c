/* SPDX-License-Identifier: GPL-2.0-or-later */
/* Original fan policy, test oracle only. Broad source snapshot: main b0ef6ec. */
#include <stdio.h>
#include <stdbool.h>
#include <limits.h>
#define ACER_FAN_CURVE_CUSTOM 2
#define ACER_FAN_CURVE_AGGRESSIVE 1
#define ACER_FAN_CURVE_HYSTERESIS 3
#define ACER_FAN_CURVE_MAX_STEP 15
#define max(a,b) ((a)>(b)?(a):(b))
struct acer_fan_curve_point { int temp, cpu, gpu; };
struct acer_fan_curve_point custom_fan_curve[]={{55,0,0},{65,35,35},{75,55,60},{85,80,85},{90,100,100}};
int custom_fan_curve_points=5, fan_curve_profile;
int curve_cpu_last_temp=INT_MIN,curve_gpu_last_temp=INT_MIN;
int curve_cpu_last_speed=-1,curve_gpu_last_speed=-1;
static int acer_custom_fan_percent_for_temp(int temp, bool cpu_fan)
 {
     int i;

     if (temp <= custom_fan_curve[0].temp)
         return cpu_fan ? custom_fan_curve[0].cpu : custom_fan_curve[0].gpu;

     for (i = 1; i < custom_fan_curve_points; i++) {
         struct acer_fan_curve_point *prev = &custom_fan_curve[i - 1];
         struct acer_fan_curve_point *next = &custom_fan_curve[i];
         int prev_speed = cpu_fan ? prev->cpu : prev->gpu;
         int next_speed = cpu_fan ? next->cpu : next->gpu;

         if (temp <= next->temp)
             return prev_speed + (next_speed - prev_speed) *
                    (temp - prev->temp) / (next->temp - prev->temp);
     }

     return cpu_fan ? custom_fan_curve[custom_fan_curve_points - 1].cpu :
              custom_fan_curve[custom_fan_curve_points - 1].gpu;
 }
static int acer_fan_percent_for_temp(int temp, bool cpu_fan)
 {
     if (fan_curve_profile == ACER_FAN_CURVE_CUSTOM)
         return acer_custom_fan_percent_for_temp(temp, cpu_fan);

     if (fan_curve_profile == ACER_FAN_CURVE_AGGRESSIVE) {
         if (temp < 55)
             return 0;
         if (temp < 65)
             return 40;
         if (temp < 75)
             return 60;
         if (temp < 85)
             return 80;
         return 100;
     }

     if (temp < 60)
         return 0;
     if (temp < 65)
         return 35;
     if (temp < 75)
         return 50;
     if (temp < 80)
         return 65;
     if (temp < 90)
         return 80;
     return 100;
 }
static int acer_fan_curve_hysteresis_speed(int temp, bool cpu_fan)
 {
     int target = acer_fan_percent_for_temp(temp, cpu_fan);
     int *last_temp = cpu_fan ? &curve_cpu_last_temp : &curve_gpu_last_temp;
     int *last_speed = cpu_fan ? &curve_cpu_last_speed : &curve_gpu_last_speed;
     int speed;

     if (*last_speed < 0 || temp >= 90) {
         speed = target;
     } else if (target >= *last_speed) {
         speed = target;
     } else if (*last_temp != INT_MIN &&
                temp > *last_temp - ACER_FAN_CURVE_HYSTERESIS) {
         speed = *last_speed;
     } else {
         speed = max(target, *last_speed - ACER_FAN_CURVE_MAX_STEP);
     }

     *last_temp = temp;
     *last_speed = speed;
     return speed;
 }
int main(void) {
 for(fan_curve_profile=0;fan_curve_profile<3;fan_curve_profile++)
  for(int temperature=-20;temperature<=140;temperature++)
   for(int cpu=0;cpu<2;cpu++) printf("%d,%d,%d,%d\n",fan_curve_profile,temperature,cpu,acer_fan_percent_for_temp(temperature,cpu));
 int sequence[]={90,89,88,87,86,83,80,70,69,65,60,55,54,65,75,90};
 fan_curve_profile=0;
 for(unsigned int i=0;i<sizeof(sequence)/sizeof(sequence[0]);i++) printf("h,%d\n",acer_fan_curve_hysteresis_speed(sequence[i],true));
 return 0;
}
